//! Tests for Remove Logged Message ('R') semantics in the streaming parser.
//!
//! The ULog spec says Remove Logged marks a msg_id as "no longer being logged",
//! meaning subsequent Data messages with that msg_id should arguably be ignored.
//!
//! Current behavior: the parser parses Remove Logged messages and delivers them
//! via callback, but does NOT stop delivering Data messages for the removed msg_id.
//! This is documented as a known limitation.
//!
//! While PX4 currently does not use Remove Logged messages, the spec defines them
//! and other ULog writers might. If a topic is unsubscribed and then its msg_id is
//! reused for a different topic (spec allows this in theory), the parser would
//! deliver data with the wrong format.

mod helpers;

use helpers::ULogBuilder;
use px4_ulog::stream_parser::file_reader::{
    read_file_with_simple_callback, Message, SimpleCallbackResult,
};

/// Build a ULog stream with flag_bits, a format, and a subscription for a simple topic.
/// Returns a builder with msg_id=0 registered as "test_topic" with fields (uint64_t timestamp, float x).
fn base_builder() -> ULogBuilder {
    let mut b = ULogBuilder::new();
    b.flag_bits()
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_topic");
    b
}

/// Build a 12-byte data payload for test_topic: 8-byte timestamp + 4-byte float x.
fn make_data_payload(timestamp: u64, x: f32) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&timestamp.to_le_bytes());
    payload.extend_from_slice(&x.to_le_bytes());
    payload
}

/// Helper: write bytes to a temp file, parse with simple callback, return collected messages.
/// Each message is tagged with a string for easy assertion.
fn parse_to_tags(bytes: &[u8], test_name: &str) -> Vec<String> {
    let tmp = std::env::temp_dir().join(format!("{}.ulg", test_name));
    std::fs::write(&tmp, bytes).expect("write temp file");

    let mut tags = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        match msg {
            Message::Data(d) => tags.push(format!("Data(msg_id={})", d.msg_id)),
            Message::RemoveLoggedMessage(r) => tags.push(format!("Remove(msg_id={})", r.msg_id)),
            Message::LoggedMessage(_) => tags.push("LoggedMessage".to_string()),
            Message::ParameterMessage(_) => tags.push("ParameterMessage".to_string()),
            Message::InfoMessage(_) => tags.push("InfoMessage".to_string()),
            Message::DropoutMessage(_) => tags.push("DropoutMessage".to_string()),
            Message::SyncMessage(_) => tags.push("SyncMessage".to_string()),
            Message::MultiInfoMessage(_) => tags.push("MultiInfoMessage".to_string()),
            Message::TaggedLoggedMessage(_) => tags.push("TaggedLoggedMessage".to_string()),
            Message::ParameterDefaultMessage(_) => tags.push("ParameterDefaultMessage".to_string()),
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse without error");

    let _ = std::fs::remove_file(&tmp);
    tags
}

// =============================================================================
// Test 1: Data after Remove is still delivered (known limitation)
// =============================================================================

/// After a Remove Logged message for msg_id=0, subsequent Data messages for
/// msg_id=0 are still delivered to the callback.
///
/// KNOWN LIMITATION: The parser does not track removed msg_ids and therefore
/// cannot filter out Data messages for removed subscriptions. Implementing
/// this filtering would require maintaining a HashSet<u16> of removed msg_ids
/// in the parser's DataFormat struct and checking it in the Data handler.
///
/// This matters if a ULog writer unsubscribes a topic and reuses the msg_id
/// for a different topic -- the parser would deliver data with the old format.
#[test]
fn test_data_after_remove_still_delivered() {
    let mut b = base_builder();

    // Data before remove
    b.data(0, &make_data_payload(1000, 1.0));

    // Remove msg_id=0
    b.remove_logged(0);

    // Data after remove -- currently still delivered (known limitation)
    b.data(0, &make_data_payload(2000, 2.0));

    let tags = parse_to_tags(&b.build(), "test_data_after_remove_still_delivered");

    // Verify the Remove message was delivered
    assert!(
        tags.contains(&"Remove(msg_id=0)".to_string()),
        "Remove message should be delivered to callback"
    );

    // Count Data messages for msg_id=0
    let data_count = tags.iter().filter(|t| *t == "Data(msg_id=0)").count();

    // KNOWN LIMITATION: Both Data messages are delivered, even the one after Remove.
    // If filtering were implemented, this count would be 1 (only pre-remove data).
    // Currently it is 2 because the parser does not suppress post-remove data.
    assert_eq!(
        data_count, 2,
        "Expected 2 Data messages (known limitation: post-remove data is not filtered). \
         If this fails with count=1, filtering has been implemented -- update this test!"
    );
}

// =============================================================================
// Test 2: Remove does not affect other msg_ids
// =============================================================================

/// Removing msg_id=0 should not affect delivery of Data messages for msg_id=1.
#[test]
fn test_remove_does_not_affect_other_msg_ids() {
    let mut b = ULogBuilder::new();
    b.flag_bits()
        .format("topic_a", &[("uint64_t", "timestamp"), ("float", "x")])
        .format("topic_b", &[("uint64_t", "timestamp"), ("float", "y")])
        .add_logged(0, 0, "topic_a")
        .add_logged(1, 0, "topic_b");

    // Data for both topics
    b.data(0, &make_data_payload(1000, 1.0));
    b.data(1, &make_data_payload(1000, 10.0));

    // Remove only msg_id=0
    b.remove_logged(0);

    // More data for msg_id=1 -- should still be delivered
    b.data(1, &make_data_payload(2000, 20.0));

    let tags = parse_to_tags(&b.build(), "test_remove_does_not_affect_other_msg_ids");

    // msg_id=1 should have 2 Data messages
    let data_1_count = tags.iter().filter(|t| *t == "Data(msg_id=1)").count();
    assert_eq!(
        data_1_count, 2,
        "Remove of msg_id=0 should not affect msg_id=1 data delivery"
    );

    // Remove message should be present
    assert!(
        tags.contains(&"Remove(msg_id=0)".to_string()),
        "Remove message for msg_id=0 should be delivered"
    );

    // msg_id=0 should have at least 1 Data message (the pre-remove one)
    let data_0_count = tags.iter().filter(|t| *t == "Data(msg_id=0)").count();
    assert!(
        data_0_count >= 1,
        "msg_id=0 should have at least the pre-remove Data message"
    );
}

// =============================================================================
// Test 3: Remove message itself is delivered to callback
// =============================================================================

/// The Remove Logged message should reach the callback as Message::RemoveLoggedMessage
/// with the correct msg_id.
#[test]
fn test_remove_message_is_delivered_to_callback() {
    let mut b = base_builder();
    b.data(0, &make_data_payload(1000, 1.0));
    b.remove_logged(0);

    let tmp = std::env::temp_dir().join("test_remove_message_callback.ulg");
    std::fs::write(&tmp, b.build()).expect("write temp file");

    let mut remove_msg_ids: Vec<u16> = Vec::new();

    read_file_with_simple_callback(tmp.to_str().unwrap(), &mut |msg| {
        if let Message::RemoveLoggedMessage(r) = msg {
            remove_msg_ids.push(r.msg_id);
        }
        SimpleCallbackResult::KeepReading
    })
    .expect("should parse without error");

    let _ = std::fs::remove_file(&tmp);

    assert_eq!(
        remove_msg_ids.len(),
        1,
        "Expected exactly one Remove message"
    );
    assert_eq!(remove_msg_ids[0], 0, "Remove message should have msg_id=0");
}

// =============================================================================
// Test 4: Remove for a never-registered msg_id does not error
// =============================================================================

/// Sending a Remove Logged message for a msg_id that was never registered via
/// AddLogged should not cause a parse error. The parser should silently accept it.
#[test]
fn test_remove_for_never_registered_msg_id() {
    let mut b = ULogBuilder::new();
    b.flag_bits()
        .format("test_topic", &[("uint64_t", "timestamp"), ("float", "x")])
        .add_logged(0, 0, "test_topic");

    // Data for the registered msg_id
    b.data(0, &make_data_payload(1000, 1.0));

    // Remove a msg_id that was never registered (msg_id=99)
    b.remove_logged(99);

    // More data for the registered msg_id -- should still work fine
    b.data(0, &make_data_payload(2000, 2.0));

    let tags = parse_to_tags(&b.build(), "test_remove_never_registered");

    // Should not have errored -- we got here
    // The Remove message should still be delivered to the callback
    assert!(
        tags.contains(&"Remove(msg_id=99)".to_string()),
        "Remove for unregistered msg_id=99 should still be delivered to callback"
    );

    // Registered msg_id=0 data should be unaffected
    let data_count = tags.iter().filter(|t| *t == "Data(msg_id=0)").count();
    assert_eq!(
        data_count, 2,
        "Data for msg_id=0 should be unaffected by removing unregistered msg_id=99"
    );
}
