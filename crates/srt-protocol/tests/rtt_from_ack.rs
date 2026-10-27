//! A sender learns the round-trip time from the receiver's ACKs, as libsrt's does. Only the side
//! sending ACKs gets ACKACKs to measure from, so a pure sender has no samples of its own, and its
//! retransmit gate (one RTT) and timeouts would run on the 100 ms initial value.

mod sim;

use std::time::Duration;

use sim::{LinkConfig, Pair, t0};
use srt_protocol::connection::{Config, Connection};
use srt_protocol::listener::Listener;
use srt_protocol::packet::SocketId;
use srt_protocol::seq::SeqNumber;

fn link(one_way_ms: u64) -> LinkConfig {
    LinkConfig {
        delay: Duration::from_millis(one_way_ms),
        ..LinkConfig::PERFECT
    }
}

fn connected(one_way_ms: u64) -> Pair {
    let config = || {
        Config::default()
            .with_latency(Duration::from_millis(400))
            .with_flow_window(8192)
    };
    let now = t0();
    let caller = Connection::connect(
        config(),
        SocketId::new(0x11),
        SeqNumber::new(1000),
        now,
        |_| {},
    );
    let listener = Listener::new(
        config(),
        SocketId::new(0x22),
        SeqNumber::new(9000),
        0xC0FF_EE12,
        now,
    );
    let mut pair = Pair::new(now, caller, listener, link(one_way_ms), link(one_way_ms), 1);
    assert!(pair.run_until_connected(400), "handshake completes");
    pair
}

/// Sends a packet every millisecond for `ms` milliseconds.
fn stream(pair: &mut Pair, ms: u64) {
    for i in 0..ms {
        pair.caller_send(&i.to_be_bytes());
        pair.run_for(1_000);
    }
}

#[test]
fn a_pure_sender_takes_the_rtt_from_the_receivers_acks() {
    let mut pair = connected(20);
    stream(&mut pair, 1_000);
    let receiver = pair.accepted_stats().expect("accepted").rtt_us;
    let sender = pair.caller_stats().rtt_us;
    // 20 ms each way.
    assert!(
        (38_000..=42_000).contains(&receiver),
        "the receiver measures it: {receiver} µs"
    );
    assert!(
        (38_000..=42_000).contains(&sender),
        "and the sender learns it: {sender} µs"
    );
}

#[test]
fn the_sender_follows_a_changing_rtt() {
    // The sender's one RTT sample of its own (its single ACK, at connect) is taken on a 10 ms
    // path; then the path gets longer. It must follow the receiver, not keep that sample.
    let mut pair = connected(10);
    stream(&mut pair, 300);
    pair.degrade_links(link(60), link(60), 2);
    stream(&mut pair, 1_500);
    let receiver = pair.accepted_stats().expect("accepted").rtt_us;
    let sender = pair.caller_stats().rtt_us;
    assert!(
        (110_000..=125_000).contains(&receiver),
        "the receiver measures 120 ms: {receiver} µs"
    );
    assert!(
        (110_000..=125_000).contains(&sender),
        "and the sender follows: {sender} µs"
    );
}
