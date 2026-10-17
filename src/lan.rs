use crate::{
    config::{DeviceConfig, DiscoveryConfig},
    device::Device,
};
use anyhow::{Context, Result, bail, ensure};
use std::{collections::BTreeSet, net::Ipv4Addr};

#[derive(Clone, Copy)]
struct Network {
    address: Ipv4Addr,
    mask: u32,
    interface: u32,
}

impl Network {
    fn contains(&self, address: Ipv4Addr) -> bool {
        u32::from(address) & self.mask == u32::from(self.address) & self.mask
    }

    fn targets(&self, old: Ipv4Addr) -> Vec<Ipv4Addr> {
        let anchor = if self.contains(old) {
            old
        } else {
            self.address
        };
        // Bound discovery to one /24 (or the actual subnet when smaller).
        let mask = self.mask | 0xffff_ff00;
        let network = u32::from(anchor) & mask;
        let broadcast = network | !mask;
        (network.saturating_add(1)..broadcast)
            .map(Ipv4Addr::from)
            .filter(|address| *address != self.address)
            .collect()
    }
}

fn choose_network(networks: Vec<Network>, old: Ipv4Addr) -> Result<Network> {
    let mut eligible: Vec<_> = networks
        .into_iter()
        .filter(|n| {
            n.address.is_private() && n.mask != 0 && n.mask.leading_ones() == n.mask.count_ones()
        })
        .collect();
    let on_link: Vec<_> = eligible
        .iter()
        .filter(|network| network.contains(old))
        .copied()
        .collect();
    if on_link.len() == 1 {
        return Ok(on_link[0]);
    }
    ensure!(
        on_link.is_empty() && eligible.len() == 1,
        "cannot choose one connected LAN for Pixoo discovery"
    );
    Ok(eligible.remove(0))
}

fn matching_addresses(
    rows: Vec<(u32, Ipv4Addr, String)>,
    network: Network,
    mac: &str,
) -> Vec<Ipv4Addr> {
    rows.into_iter()
        .filter(|(interface, address, candidate)| {
            *interface == network.interface
                && address.is_private()
                && network.contains(*address)
                && DiscoveryConfig::normalize_mac(candidate).as_deref() == Some(mac)
        })
        .map(|(_, address, _)| address)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[cfg(windows)]
mod native {
    use super::*;
    use windows_sys::Win32::NetworkManagement::IpHelper::{
        GetIpAddrTable, GetIpNetTable, MIB_IPADDRROW_XP, MIB_IPADDRTABLE, MIB_IPNETROW_LH,
        MIB_IPNETTABLE,
    };

    fn rows<T: Copy>(offset: usize, read: impl Fn(*mut u8, *mut u32) -> u32) -> Result<Vec<T>> {
        let mut size = 0;
        let initial = read(std::ptr::null_mut(), &mut size);
        if initial == 232 {
            return Ok(Vec::new());
        } // ERROR_NO_DATA
        ensure!(
            initial == 122,
            "read Windows network table: {}",
            std::io::Error::from_raw_os_error(initial as i32)
        );
        for _ in 0..3 {
            ensure!(
                size > 0 && size <= 1024 * 1024,
                "invalid Windows network table size"
            );
            // Both table types contain only scalar fields and need <=8-byte alignment.
            let mut buffer = vec![0_u64; (size as usize).div_ceil(8)];
            let capacity = buffer.len() * 8;
            let code = read(buffer.as_mut_ptr().cast(), &mut size);
            if code == 122 {
                continue;
            }
            if code == 232 {
                return Ok(Vec::new());
            }
            ensure!(
                code == 0,
                "read Windows network table: {}",
                std::io::Error::from_raw_os_error(code as i32)
            );
            ensure!(
                size as usize <= capacity && size as usize >= offset,
                "invalid network table length"
            );
            let count = unsafe { buffer.as_ptr().cast::<u32>().read_unaligned() } as usize;
            ensure!(
                count <= (size as usize - offset) / std::mem::size_of::<T>(),
                "invalid network table row count"
            );
            // The OS wrote these Copy-only rows; offsets use the generated SDK layouts.
            let data = unsafe { buffer.as_ptr().cast::<u8>().add(offset).cast::<T>() };
            return Ok((0..count)
                .map(|i| unsafe { data.add(i).read_unaligned() })
                .collect());
        }
        bail!("Windows network table changed repeatedly during discovery")
    }

    pub(super) fn networks() -> Result<Vec<Network>> {
        let rows = rows::<MIB_IPADDRROW_XP>(
            std::mem::offset_of!(MIB_IPADDRTABLE, table),
            |data, size| unsafe { GetIpAddrTable(data.cast(), size, 0) },
        )?;
        Ok(rows
            .into_iter()
            .filter(|r| r.wType & 0x48 == 0)
            .map(|r| Network {
                address: Ipv4Addr::from(r.dwAddr.to_ne_bytes()),
                mask: u32::from(Ipv4Addr::from(r.dwMask.to_ne_bytes())),
                interface: r.dwIndex,
            })
            .collect())
    }

    pub(super) fn neighbors() -> Result<Vec<(u32, Ipv4Addr, String)>> {
        let rows = rows::<MIB_IPNETROW_LH>(
            std::mem::offset_of!(MIB_IPNETTABLE, table),
            |data, size| unsafe { GetIpNetTable(data.cast(), size, 0) },
        )?;
        Ok(rows
            .into_iter()
            .filter(|r| r.dwPhysAddrLen == 6 && unsafe { r.Anonymous.dwType } != 2)
            .map(|r| {
                (
                    r.dwIndex,
                    Ipv4Addr::from(r.dwAddr.to_ne_bytes()),
                    r.bPhysAddr[..6]
                        .iter()
                        .map(|byte| format!("{byte:02x}"))
                        .collect(),
                )
            })
            .collect())
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn network_table_reader_rejects_invalid_lengths_and_handles_no_data_or_denial() {
            let offset = std::mem::offset_of!(MIB_IPADDRTABLE, table);
            let error = rows::<MIB_IPADDRROW_XP>(offset, |data, size| {
                unsafe {
                    *size = offset as u32;
                }
                if data.is_null() {
                    return 122;
                }
                unsafe {
                    data.cast::<u32>().write_unaligned(u32::MAX);
                }
                0
            })
            .err()
            .unwrap();
            assert!(error.to_string().contains("row count"));
            assert!(
                rows::<MIB_IPADDRROW_XP>(offset, |_, _| 232)
                    .unwrap()
                    .is_empty()
            );
            assert!(rows::<MIB_IPADDRROW_XP>(offset, |_, _| 5).is_err());
        }

        #[test]
        fn network_table_reader_uses_sdk_row_offsets_and_network_byte_order() {
            let offset = std::mem::offset_of!(MIB_IPADDRTABLE, table);
            let size_bytes = offset + std::mem::size_of::<MIB_IPADDRROW_XP>();
            let rows = rows::<MIB_IPADDRROW_XP>(offset, |data, size| {
                unsafe {
                    *size = size_bytes as u32;
                }
                if data.is_null() {
                    return 122;
                }
                let row = MIB_IPADDRROW_XP {
                    dwAddr: u32::from_ne_bytes([192, 168, 1, 17]),
                    dwMask: u32::from_ne_bytes([255, 255, 255, 0]),
                    dwIndex: 19,
                    ..Default::default()
                };
                unsafe {
                    data.cast::<u32>().write_unaligned(1);
                    data.add(offset)
                        .cast::<MIB_IPADDRROW_XP>()
                        .write_unaligned(row);
                }
                0
            })
            .unwrap();
            assert_eq!(rows.len(), 1);
            assert_eq!(
                Ipv4Addr::from(rows[0].dwAddr.to_ne_bytes()),
                "192.168.1.17".parse::<Ipv4Addr>().unwrap()
            );
            assert_eq!(
                u32::from(Ipv4Addr::from(rows[0].dwMask.to_ne_bytes())),
                0xffff_ff00
            );
        }
    }
}

#[cfg(windows)]
pub async fn find(config: &DeviceConfig) -> Result<String> {
    use tokio::{
        net::TcpStream,
        task::JoinSet,
        time::{Duration, timeout},
    };
    let mac = config
        .discovery
        .mac_address
        .as_deref()
        .and_then(DiscoveryConfig::normalize_mac)
        .context("local discovery needs the Pixoo's saved MAC address")?;
    let url = reqwest::Url::parse(&config.url()?)?;
    let old: Ipv4Addr = url
        .host_str()
        .context("device host missing")?
        .parse()
        .context("local discovery needs a configured IPv4 address")?;
    ensure!(
        old.is_private(),
        "configured address is not a private LAN address"
    );
    let network = choose_network(native::networks()?, old)?;
    let port = url.port_or_known_default().unwrap_or(80);
    for refresh in [false, true] {
        if refresh {
            let mut jobs = JoinSet::new();
            for address in network.targets(old) {
                if jobs.len() >= 24 {
                    let _ = jobs.join_next().await;
                }
                jobs.spawn(async move {
                    let _ = timeout(
                        Duration::from_millis(500),
                        TcpStream::connect((address, port)),
                    )
                    .await;
                });
            }
            while jobs.join_next().await.is_some() {}
        }
        let candidates = matching_addresses(native::neighbors()?, network, &mac);
        ensure!(
            candidates.len() <= 4,
            "too many LAN addresses claim the configured MAC address"
        );
        let mut healthy = Vec::new();
        for address in candidates {
            // Send the local token only after the neighbor MAC matched the pinned device.
            let mut candidate = config.clone();
            candidate.address = if url.port().is_some() {
                format!("{address}:{port}")
            } else {
                address.to_string()
            };
            candidate.timeout_ms = candidate.timeout_ms.min(2000);
            if Device::new(candidate.clone())?.probe().await.is_ok() {
                healthy.push(candidate.address);
            }
        }
        ensure!(
            healthy.len() <= 1,
            "multiple reachable LAN addresses claim the Pixoo MAC address"
        );
        if let Some(address) = healthy.pop() {
            return Ok(address);
        }
    }
    bail!("the saved Pixoo MAC address was not reachable on the connected LAN")
}

#[cfg(not(windows))]
pub async fn find(_config: &DeviceConfig) -> Result<String> {
    bail!("local MAC discovery is supported on Windows")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn network(ip: &str, mask: u32, interface: u32) -> Network {
        Network {
            address: ip.parse().unwrap(),
            mask,
            interface,
        }
    }

    #[test]
    fn scan_is_bounded_to_connected_private_network_and_excludes_self_and_broadcast() {
        let n = network("192.168.1.17", 0xffff_ff00, 19);
        let targets = n.targets("192.168.1.30".parse().unwrap());
        assert_eq!(targets.len(), 253);
        assert!(!targets.contains(&n.address));
        assert!(!targets.contains(&"192.168.1.0".parse().unwrap()));
        assert!(!targets.contains(&"192.168.1.255".parse().unwrap()));
        assert!(targets.contains(&"192.168.1.18".parse().unwrap()));
        assert!(
            network("10.0.0.1", 0xffff_0000, 1)
                .targets("10.0.5.6".parse().unwrap())
                .len()
                <= 254
        );
        assert_eq!(
            network("192.168.1.17", 0xffff_fff0, 19)
                .targets("192.168.1.18".parse().unwrap())
                .len(),
            13
        );
    }

    #[test]
    fn selects_correct_interface_and_refuses_ambiguous_networks() {
        let old = "192.168.1.30".parse().unwrap();
        let n = choose_network(
            vec![
                network("172.17.0.1", 0xffff_0000, 1),
                network("192.168.1.17", 0xffff_ff00, 19),
            ],
            old,
        )
        .unwrap();
        assert_eq!(n.interface, 19);
        assert!(choose_network(vec![n, n], old).is_err());
        assert!(choose_network(vec![network("8.8.8.8", 0xffff_ff00, 1)], old).is_err());
        assert!(choose_network(vec![], old).is_err());
        assert_eq!(
            choose_network(vec![network("10.0.0.17", 0xffff_ff00, 19)], old)
                .unwrap()
                .interface,
            19
        );
    }

    #[test]
    fn local_match_requires_the_right_interface_subnet_and_exact_mac() {
        let n = network("192.168.1.17", 0xffff_ff00, 19);
        let rows = vec![
            (
                19,
                "192.168.1.18".parse().unwrap(),
                "02:11:22:33:44:55".into(),
            ),
            (19, "192.168.1.19".parse().unwrap(), "001122334455".into()),
            (5, "192.168.1.20".parse().unwrap(), "021122334455".into()),
            (19, "192.168.2.18".parse().unwrap(), "021122334455".into()),
        ];
        assert_eq!(
            matching_addresses(rows, n, "021122334455"),
            vec!["192.168.1.18".parse::<Ipv4Addr>().unwrap()]
        );
    }
}
