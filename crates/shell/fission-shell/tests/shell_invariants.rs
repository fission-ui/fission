use fission_shell::{
    InputEvent, LifecycleEvent, LayoutPoint, LayoutSize, PointerEvent, PointerButton, Platform,
};
use anyhow::Result;
use serde_json;

// --- Dummy Platform for testing trait implementation ---
pub struct DummyPlatform;

impl Platform for DummyPlatform {
    fn dispatch_event(&mut self, event: InputEvent) -> Result<()> {
        // In a real scenario, this would forward to the Fission Core Runtime
        // println!("DummyPlatform received event: {:?}", event);
        Ok(())
    }

    fn present(&mut self, _display_list_data: &[u8]) -> Result<()> {
        // println!("DummyPlatform presenting display list data.");
        Ok(())
    }
}

#[test]
fn test_input_event_serialization() {
    let event1 = InputEvent::Pointer(PointerEvent::Down {
        point: LayoutPoint { x: 100.0, y: 50.0 },
        button: PointerButton::Primary,
    });
    let event2 = InputEvent::Lifecycle(LifecycleEvent::Resize {
        size: LayoutSize { width: 800.0, height: 600.0 },
    });

    let json1 = serde_json::to_string(&event1).unwrap();
    let deserialized1: InputEvent = serde_json::from_str(&json1).unwrap();
    assert_eq!(event1, deserialized1);

    let json2 = serde_json::to_string(&event2).unwrap();
    let deserialized2: InputEvent = serde_json::from_str(&json2).unwrap();
    assert_eq!(event2, deserialized2);
}

#[test]
fn test_platform_trait_object() -> Result<()> {
    let mut platform: Box<dyn Platform> = Box::new(DummyPlatform);
    let event = InputEvent::Lifecycle(LifecycleEvent::Init);
    let dl_data = vec![1, 2, 3, 4]; // Dummy display list data

    // Verify that methods can be called through the trait object
    platform.dispatch_event(event)?;
    platform.present(&dl_data)?;
    Ok(())
}
