use super::*;

#[test]
fn android_volume_percent_mapping_is_bounded() {
    assert_eq!(percent_to_platform_volume(50, 15), 8);
    assert_eq!(platform_volume_to_percent(20, 10), 100);
    assert_eq!(platform_volume_to_percent(0, 0), 0);
}

#[test]
fn android_wifi_security_parses_common_capability_strings() {
    assert_eq!(
        android_wifi_security("[WPA2-PSK-CCMP][ESS]"),
        WifiSecurity::Wpa2
    );
    assert_eq!(android_wifi_security("[SAE][ESS]"), WifiSecurity::Wpa3);
    assert_eq!(android_wifi_security("[WEP][ESS]"), WifiSecurity::Wep);
    assert_eq!(android_wifi_security("[ESS]"), WifiSecurity::Open);
}

#[test]
fn notification_ids_are_stable_positive_values() {
    assert_eq!(
        notification_id_to_i32(&NotificationId::new("sync-complete")),
        630_319_610
    );
    assert!(notification_id_to_i32(&NotificationId::new("x")) >= 0);
}
