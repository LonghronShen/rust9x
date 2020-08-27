const WINDOWS_VERSIONS: [&str; 26] = [
    "3.10.511",   // NT 3.1
    "3.10.528",   // NT 3.1 SP3
    "3.50.807",   // NT 3.5
    "3.51.1057",  // NT 3.51
    "4.0.1381",   // NT 4
    "5.0.2195",   // 2000
    "5.1.2600",   // XP
    "5.2.3790",   // XP 64bit, Server 2003
    "6.0.6000",   // Vista, Server 2008
    "6.0.6001",   // Vista SP1, Server 2008 SP1
    "6.0.6002",   // Vista SP2, Server 2008 SP2
    "6.1.7600",   // 7, Server 2008 R2
    "6.1.7601",   // 7 SP1, Server 2008 R2 SP1
    "6.1.8400",   // Home Server 2011
    "6.2.9200",   // 8, Server 2012
    "6.3.9600",   // 8.1, Server 2012 R2
    "10.0.10240", // 10 1507
    "10.0.10586", // 10 1511
    "10.0.14393", // 10 1607, Server 2016 1607
    "10.0.15063", // 10 1703
    "10.0.16299", // 10 1709, Server 2016 1709
    "10.0.17134", // 10 1803
    "10.0.17763", // 10 1809, Server 2019 1809
    "10.0.18362", // 10 1903
    "10.0.18363", // 10 1909
    "10.0.19041", // 10 2004
];

const ANSI_WINDOWS_VERSIONS: [&str; 7] = [
    "4.00.950",   // 95
    "4.00.950 A", // 95 OSR1 (A)
    "4.00.950 B", // 95 OSR2/2.1 (B)
    "4.00.950 C", // 95 OSR2.5 (C)
    "4.10.1998",  // 98
    "4.10.2222",  // 98 SE
    "4.90.3000",  // ME
];

pub fn get_enabled_target_api_features(requested_apis: &[String]) -> Vec<&'static str> {
    fn find_highest_index(
        requested_apis: &[String],
        version_list: &[&'static str],
    ) -> Option<usize> {
        requested_apis.iter().fold(None, |old_highest_index, api| {
            if let Some((index, _)) = version_list.iter().enumerate().find(|&(_, ver)| ver == api) {
                old_highest_index.map(|old_index| std::cmp::max(old_index, index)).or(Some(index))
            } else {
                old_highest_index
            }
        })
    }

    let highest_index_nt = find_highest_index(requested_apis, &WINDOWS_VERSIONS);
    let highest_index_ansi = find_highest_index(requested_apis, &ANSI_WINDOWS_VERSIONS);

    let mut enabled_features =
        highest_index_nt.map(|i| WINDOWS_VERSIONS[..=i].into()).unwrap_or(vec![]);

    if let Some(i) = highest_index_ansi {
        enabled_features.extend(&ANSI_WINDOWS_VERSIONS[..=i]);
    }

    enabled_features
}
