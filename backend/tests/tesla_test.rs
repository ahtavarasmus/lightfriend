//! Tests for Tesla client trait and pure functions

use backend::{
    format_battery_status, is_actively_charging, is_charging_complete, is_charging_stopped,
    is_climate_ready, should_wake_vehicle, wake_retry_delay,
};
use std::time::Duration;

// ============================================================
// Pure Function Tests: should_wake_vehicle
// ============================================================

#[test]
fn test_should_wake_vehicle_asleep() {
    assert!(should_wake_vehicle("asleep"));
}

#[test]
fn test_should_wake_vehicle_offline() {
    assert!(should_wake_vehicle("offline"));
}

#[test]
fn test_should_wake_vehicle_online() {
    assert!(!should_wake_vehicle("online"));
}

#[test]
fn test_should_wake_vehicle_unknown_state() {
    assert!(should_wake_vehicle("suspended"));
    assert!(should_wake_vehicle("unknown"));
    assert!(should_wake_vehicle(""));
}

// ============================================================
// Pure Function Tests: wake_retry_delay
// ============================================================

#[test]
fn test_wake_retry_delay_attempt_0() {
    assert_eq!(wake_retry_delay(0), Duration::from_secs(1)); // 2^0 = 1
}

#[test]
fn test_wake_retry_delay_attempt_1() {
    assert_eq!(wake_retry_delay(1), Duration::from_secs(2)); // 2^1 = 2
}

#[test]
fn test_wake_retry_delay_attempt_2() {
    assert_eq!(wake_retry_delay(2), Duration::from_secs(4)); // 2^2 = 4
}

#[test]
fn test_wake_retry_delay_attempt_5() {
    assert_eq!(wake_retry_delay(5), Duration::from_secs(32)); // 2^5 = 32
}

#[test]
fn test_wake_retry_delay_caps_at_5() {
    // Attempts > 5 should be capped at 2^5 = 32 seconds
    assert_eq!(wake_retry_delay(6), Duration::from_secs(32));
    assert_eq!(wake_retry_delay(10), Duration::from_secs(32));
    assert_eq!(wake_retry_delay(100), Duration::from_secs(32));
}

// ============================================================
// Pure Function Tests: is_climate_ready
// ============================================================

#[test]
fn test_is_climate_ready_at_target() {
    // Inside temp equals target, should be ready
    assert!(is_climate_ready(21.0, 21.0, 3.0));
}

#[test]
fn test_is_climate_ready_within_tolerance() {
    // Inside temp within tolerance of target
    assert!(is_climate_ready(19.0, 21.0, 3.0)); // 21 - 19 = 2 <= 3
    assert!(is_climate_ready(18.0, 21.0, 3.0)); // 21 - 18 = 3 <= 3
}

#[test]
fn test_is_climate_ready_outside_tolerance() {
    // Inside temp outside tolerance AND below minimum comfortable temp
    assert!(!is_climate_ready(10.0, 21.0, 3.0)); // 21 - 10 = 11 > 3, 10 < 15
    assert!(!is_climate_ready(5.0, 21.0, 3.0)); // 21 - 5 = 16 > 3, 5 < 15
}

#[test]
fn test_is_climate_ready_above_minimum_comfortable() {
    // Even if outside tolerance, if above 15C it's considered comfortable
    assert!(is_climate_ready(16.0, 21.0, 2.0)); // 21 - 16 = 5 > 2, but 16 >= 15
    assert!(is_climate_ready(17.0, 21.0, 2.0)); // 21 - 17 = 4 > 2, but 17 >= 15
}

#[test]
fn test_is_climate_ready_warming_heating() {
    // Heating scenario: cold car warming up
    assert!(!is_climate_ready(-5.0, 21.0, 3.0)); // Very cold, not ready
    assert!(!is_climate_ready(10.0, 21.0, 3.0)); // Cold, not ready
    assert!(is_climate_ready(18.0, 21.0, 3.0)); // Getting warm, ready
}

#[test]
fn test_is_climate_ready_cooling() {
    // Cooling scenario: hot car cooling down
    // When target < inside, diff is negative, so it's always "within tolerance"
    assert!(is_climate_ready(35.0, 21.0, 3.0)); // Hot, but target - inside = -14 <= 3
}

// ============================================================
// Pure Function Tests: is_charging_complete
// ============================================================

#[test]
fn test_is_charging_complete_at_limit() {
    assert!(is_charging_complete(80, 80));
}

#[test]
fn test_is_charging_complete_above_limit() {
    assert!(is_charging_complete(85, 80));
    assert!(is_charging_complete(100, 90));
}

#[test]
fn test_is_charging_complete_below_limit() {
    assert!(!is_charging_complete(79, 80));
    assert!(!is_charging_complete(50, 80));
    assert!(!is_charging_complete(0, 80));
}

// ============================================================
// Pure Function Tests: is_actively_charging
// ============================================================

#[test]
fn test_is_actively_charging_true() {
    assert!(is_actively_charging("Charging"));
}

#[test]
fn test_is_actively_charging_false() {
    assert!(!is_actively_charging("Complete"));
    assert!(!is_actively_charging("Stopped"));
    assert!(!is_actively_charging("Disconnected"));
    assert!(!is_actively_charging("NoPower"));
    assert!(!is_actively_charging(""));
}

// ============================================================
// Pure Function Tests: is_charging_stopped
// ============================================================

#[test]
fn test_is_charging_stopped_complete() {
    assert!(is_charging_stopped("Complete"));
}

#[test]
fn test_is_charging_stopped_stopped() {
    assert!(is_charging_stopped("Stopped"));
}

#[test]
fn test_is_charging_stopped_disconnected() {
    assert!(is_charging_stopped("Disconnected"));
}

#[test]
fn test_is_charging_stopped_no_power() {
    assert!(is_charging_stopped("NoPower"));
}

#[test]
fn test_is_charging_stopped_charging() {
    assert!(!is_charging_stopped("Charging"));
}

#[test]
fn test_is_charging_stopped_starting() {
    assert!(!is_charging_stopped("Starting"));
}

// ============================================================
// Pure Function Tests: format_battery_status
// ============================================================

#[test]
fn test_format_battery_status_not_charging() {
    let result = format_battery_status("Model 3", 75, 220.0, 80, "Stopped", None);
    assert!(result.contains("Model 3"));
    assert!(result.contains("75%"));
    assert!(result.contains("220 miles"));
    assert!(result.contains("80%"));
    assert!(!result.contains("Currently charging"));
}

#[test]
fn test_format_battery_status_charging() {
    let result = format_battery_status("Model Y", 60, 180.0, 90, "Charging", Some(45));
    assert!(result.contains("Model Y"));
    assert!(result.contains("60%"));
    assert!(result.contains("180 miles"));
    assert!(result.contains("90%"));
    assert!(result.contains("Currently charging"));
    assert!(result.contains("45 minutes"));
}

#[test]
fn test_format_battery_status_charging_no_time() {
    let result = format_battery_status("Tesla", 50, 150.0, 80, "Charging", None);
    assert!(result.contains("Currently charging"));
    assert!(result.contains("0 minutes")); // None becomes 0
}

// ============================================================
// Mock Client Tests (require tokio runtime)
// ============================================================

#[cfg(test)]
mod mock_client_tests {
    use backend::api::tesla_client::mock::{MockTeslaClient, TeslaCommand};
    use backend::api::tesla_client::{create_test_vehicle, create_test_vehicle_with_data};
    use backend::TeslaClientInterface;

    #[tokio::test]
    async fn test_mock_client_get_vehicles() {
        let mock = MockTeslaClient::new()
            .with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"))
            .with_vehicle(create_test_vehicle("VIN456", "Model Y", "asleep"));

        let vehicles = mock.get_vehicles("token").await.unwrap();
        assert_eq!(vehicles.len(), 2);
        assert_eq!(vehicles[0].vin, "VIN123");
        assert_eq!(vehicles[1].vin, "VIN456");
    }

    #[tokio::test]
    async fn test_mock_client_get_vehicle_data() {
        let mock = MockTeslaClient::new().with_vehicle(create_test_vehicle_with_data(
            "VIN123", "Model 3", "online", 80, 21.0,
        ));

        let vehicle = mock.get_vehicle_data("token", "VIN123").await.unwrap();
        assert_eq!(vehicle.vin, "VIN123");
        assert!(vehicle.charge_state.is_some());
        assert_eq!(vehicle.charge_state.unwrap().battery_level, 80);
    }

    #[tokio::test]
    async fn test_mock_client_lock_unlock_commands() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        // Execute lock command
        let result = mock.lock_vehicle("token", "VIN123").await.unwrap();
        assert!(result);

        // Execute unlock command
        let result = mock.unlock_vehicle("token", "VIN123").await.unwrap();
        assert!(result);

        // Verify commands were logged
        let commands = mock.get_commands().await;
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], TeslaCommand::Lock);
        assert_eq!(commands[1], TeslaCommand::Unlock);
    }

    #[tokio::test]
    async fn test_mock_client_climate_commands() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        // Start climate
        let result = mock.start_climate("token", "VIN123").await.unwrap();
        assert!(result);

        // Stop climate
        let result = mock.stop_climate("token", "VIN123").await.unwrap();
        assert!(result);

        // Verify commands
        let commands = mock.get_commands().await;
        assert_eq!(commands.len(), 2);
        assert_eq!(commands[0], TeslaCommand::StartClimate);
        assert_eq!(commands[1], TeslaCommand::StopClimate);
    }

    #[tokio::test]
    async fn test_mock_client_defrost() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        let result = mock.defrost_vehicle("token", "VIN123").await.unwrap();
        assert!(result.contains("Max defrost"));

        let commands = mock.get_commands().await;
        assert_eq!(commands[0], TeslaCommand::Defrost);
    }

    #[tokio::test]
    async fn test_mock_client_failing_commands() {
        let mock = MockTeslaClient::new()
            .with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"))
            .with_failing_commands();

        // Commands should return false when configured to fail
        let result = mock.lock_vehicle("token", "VIN123").await.unwrap();
        assert!(!result);

        let result = mock.start_climate("token", "VIN123").await.unwrap();
        assert!(!result);
    }

    #[tokio::test]
    async fn test_mock_client_wake_sequence() {
        let mock = MockTeslaClient::new()
            .with_vehicle(create_test_vehicle("VIN123", "Model 3", "asleep"))
            .with_wake_sequence(vec![false, false, true]); // Fails twice, then succeeds

        // First two wake attempts fail
        let result = mock.wake_up("token", "VIN123").await.unwrap();
        assert!(!result);

        let result = mock.wake_up("token", "VIN123").await.unwrap();
        assert!(!result);

        // Third attempt succeeds
        let result = mock.wake_up("token", "VIN123").await.unwrap();
        assert!(result);

        // Subsequent attempts succeed (default behavior)
        let result = mock.wake_up("token", "VIN123").await.unwrap();
        assert!(result);
    }

    #[tokio::test]
    async fn test_mock_client_set_seat_heater() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        let result = mock.set_seat_heater("token", "VIN123", 0, 3).await.unwrap();
        assert!(result);

        let commands = mock.get_commands().await;
        assert_eq!(
            commands[0],
            TeslaCommand::SetSeatHeater {
                heater: 0,
                level: 3
            }
        );
    }

    #[tokio::test]
    async fn test_mock_client_cabin_overheat_protection() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        // Enable with A/C
        mock.set_cabin_overheat_protection("token", "VIN123", true, false)
            .await
            .unwrap();

        // Enable fan-only
        mock.set_cabin_overheat_protection("token", "VIN123", true, true)
            .await
            .unwrap();

        // Disable
        mock.set_cabin_overheat_protection("token", "VIN123", false, false)
            .await
            .unwrap();

        let commands = mock.get_commands().await;
        assert_eq!(commands.len(), 3);
        assert_eq!(
            commands[0],
            TeslaCommand::SetCabinOverheatProtection {
                on: true,
                fan_only: false
            }
        );
        assert_eq!(
            commands[1],
            TeslaCommand::SetCabinOverheatProtection {
                on: true,
                fan_only: true
            }
        );
        assert_eq!(
            commands[2],
            TeslaCommand::SetCabinOverheatProtection {
                on: false,
                fan_only: false
            }
        );
    }

    #[tokio::test]
    async fn test_mock_client_charge_limit() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        mock.set_charge_limit("token", "VIN123", 80).await.unwrap();

        let commands = mock.get_commands().await;
        assert_eq!(commands[0], TeslaCommand::SetChargeLimit(80));
    }

    #[tokio::test]
    async fn test_mock_client_share_destination() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        mock.share_destination("token", "VIN123", "37.7749,-122.4194")
            .await
            .unwrap();

        let commands = mock.get_commands().await;
        assert_eq!(
            commands[0],
            TeslaCommand::ShareDestination("37.7749,-122.4194".to_string())
        );
    }

    #[tokio::test]
    async fn test_mock_client_navigate_to_supercharger() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        mock.navigate_to_supercharger("token", "VIN123", 12345)
            .await
            .unwrap();

        let commands = mock.get_commands().await;
        assert_eq!(commands[0], TeslaCommand::NavigateToSupercharger(12345));
    }

    #[tokio::test]
    async fn test_mock_client_scheduled_departure() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        mock.set_scheduled_departure("token", "VIN123", 480, true)
            .await
            .unwrap();

        let commands = mock.get_commands().await;
        assert_eq!(
            commands[0],
            TeslaCommand::SetScheduledDeparture {
                time: 480,
                preconditioning: true
            }
        );
    }

    #[tokio::test]
    async fn test_mock_client_monitor_climate_ready() {
        let mock = MockTeslaClient::new().with_vehicle(create_test_vehicle_with_data(
            "VIN123", "Model 3", "online", 80, 21.0,
        ));

        let result = mock.monitor_climate_ready("token", "VIN123").await.unwrap();
        assert_eq!(result, Some(21.0));
    }

    #[tokio::test]
    async fn test_mock_client_monitor_charging_complete() {
        let mock = MockTeslaClient::new().with_vehicle(create_test_vehicle_with_data(
            "VIN123", "Model 3", "online", 80, 21.0,
        ));

        let result = mock
            .monitor_charging_complete("token", "VIN123")
            .await
            .unwrap();
        assert_eq!(result, Some(80));
    }

    #[tokio::test]
    async fn test_mock_client_vehicle_not_found() {
        let mock =
            MockTeslaClient::new().with_vehicle(create_test_vehicle("VIN123", "Model 3", "online"));

        let result = mock.get_vehicle_data("token", "NONEXISTENT").await;
        assert!(result.is_err());
    }
}
