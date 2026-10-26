//! Wire details libsrt depends on beyond the IETF draft (spec §3.2).

use bytes::BytesMut;
use srt_protocol::control::{Ack, AckCif, ControlBody};
use srt_protocol::loss_list::LossRange;
use srt_protocol::packet::{ControlPacket, Packet, SocketId};
use srt_protocol::seq::SeqNumber;
use srt_protocol::timestamp::Timestamp;

/// One of every control body this implementation sends.
fn bodies() -> Vec<ControlBody> {
    vec![
        ControlBody::Keepalive,
        ControlBody::Shutdown,
        ControlBody::CongestionWarning,
        ControlBody::AckAck { ack_number: 7 },
        ControlBody::PeerError { error_code: 4000 },
        ControlBody::DropReq {
            message_number: 3,
            first: SeqNumber::new(10),
            last: SeqNumber::new(12),
        },
        ControlBody::Ack(Ack {
            ack_number: 1,
            last_ack_seq: SeqNumber::new(100),
            cif: AckCif::Light,
        }),
        ControlBody::Nak {
            loss: vec![LossRange::single(SeqNumber::new(5))],
        },
    ]
}

/// libsrt 1.5.7 drops any control packet whose payload is empty or not 4-byte aligned
/// (commit fcae57145). The draft gives `Keepalive`, `Shutdown`, `AckAck`, `CongestionWarning` and
/// `PeerError` no payload, and srtrust sent them that way, so libsrt 1.5.7 ignored our shutdown,
/// ACKACKs and keepalives. libsrt itself pads them with 4 zero bytes.
#[test]
fn every_control_payload_is_non_empty_and_word_aligned() {
    for body in bodies() {
        let packet = Packet::Control(ControlPacket {
            timestamp: Timestamp::from_micros(1_000),
            dest_socket_id: SocketId::new(42),
            body: body.clone(),
        });
        let mut wire = BytesMut::new();
        packet.encode(&mut wire);
        let payload = wire.len() - 16;
        assert!(
            payload > 0 && payload.is_multiple_of(4),
            "{body:?}: {payload}-byte payload"
        );
        assert_eq!(
            Packet::decode(&wire).unwrap(),
            packet,
            "{body:?} must round-trip"
        );
    }
}

/// The padding matches libsrt's byte for byte: four zero bytes after the header.
#[test]
fn shutdown_matches_libsrt_on_the_wire() {
    let packet = Packet::Control(ControlPacket {
        timestamp: Timestamp::from_micros(0x0003_38dc),
        dest_socket_id: SocketId::new(0x2586_381f),
        body: ControlBody::Shutdown,
    });
    let mut wire = BytesMut::new();
    packet.encode(&mut wire);
    // Captured from libsrt 1.5.7 (same layout; libsrt's timestamp and socket id differ).
    assert_eq!(
        &wire[..],
        &[
            0x80, 0x05, 0x00, 0x00, // control, type 5 (shutdown), subtype 0
            0x00, 0x00, 0x00, 0x00, // type-specific
            0x00, 0x03, 0x38, 0xdc, // timestamp
            0x25, 0x86, 0x38, 0x1f, // destination socket id
            0x00, 0x00, 0x00, 0x00, // libsrt's padding
        ]
    );
}
