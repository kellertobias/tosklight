"""Offline tests: python3 -B tests/bench/release-dmx-acceptance-test.py."""
import importlib.util
import argparse
from pathlib import Path
import unittest

spec = importlib.util.spec_from_file_location("dmx_acceptance", Path(__file__).with_name("release-dmx-acceptance.py"))
dmx = importlib.util.module_from_spec(spec)
spec.loader.exec_module(dmx)


def artnet(slots=b"\x80\x00", universe=0):
    return b"Art-Net\x00\x00\x50\x00\x0e\x07\x00" + universe.to_bytes(2, "little") + len(slots).to_bytes(2, "big") + slots


def sacn(slots=b"\x80\x00", options=0):
    packet = bytearray(126 + len(slots))
    packet[:16] = b"\x00\x10\x00\x00ASC-E1.17\x00\x00\x00"
    for offset in (16, 38, 115):
        packet[offset:offset + 2] = (0x7000 | (len(packet) - offset)).to_bytes(2, "big")
    packet[18:22] = (4).to_bytes(4, "big")
    packet[22:38] = bytes(range(16))
    packet[40:44] = (2).to_bytes(4, "big")
    packet[44:48] = b"Desk"
    packet[108] = 100
    packet[111:115] = bytes([255, options, 0, 1])
    packet[117:123] = b"\x02\xa1\x00\x00\x00\x01"
    packet[123:125] = (len(slots) + 1).to_bytes(2, "big")
    packet[126:] = slots
    return bytes(packet)


class PacketTests(unittest.TestCase):
    def test_artnet_endian_and_slots(self):
        frame = dmx.decode("artnet", artnet(universe=0x1234))
        self.assertEqual((frame["universe"], frame["sequence"], frame["slots"]), (0x1234, 7, [128, 0]))

    def test_artnet_refuses_truncated_odd_oversize_and_net_bit(self):
        for packet in (artnet()[:-1], artnet(b"\x01"), artnet(bytes(514)), artnet(universe=0x8000)):
            with self.subTest(packet_length=len(packet)), self.assertRaises(ValueError):
                dmx.decode("artnet", packet)

    def test_non_dmx_artnet_not_counted(self):
        self.assertIsNone(dmx.decode("artnet", b"Art-Net\x00\x00\x20"))

    def test_sacn_offsets_and_options(self):
        frame = dmx.decode("sacn", sacn(options=0xe0))
        self.assertEqual(frame["cid"], "00010203-0405-0607-0809-0a0b0c0d0e0f")
        self.assertEqual((frame["universe"], frame["sequence"], frame["source_name"]), (1, 255, "Desk"))
        self.assertTrue(frame["preview"] and frame["terminated"] and frame["force_sync"])
        self.assertEqual(frame["slots"], [128, 0])

    def test_sacn_rejects_invalid_lengths_vectors_count_and_universe(self):
        for offset, value in ((16, 0), (40, 1), (117, 1), (124, 0), (114, 0), (108, 201), (112, 1)):
            packet = bytearray(sacn())
            packet[offset] = value
            with self.subTest(offset=offset), self.assertRaises(ValueError):
                dmx.decode("sacn", bytes(packet))
        with self.assertRaises(ValueError):
            dmx.decode("sacn", sacn()[:-1])

    def test_independent_multibyte_and_raw_assertions(self):
        frame = dict(slots=[128, 3, 0])
        case = dict(expect=[dict(slots=[1, 3], value=32768, byte_order="big"), dict(slots=[2], min=2, max=4)])
        self.assertEqual(dmx.violations(case, frame), [])
        self.assertEqual(len(dmx.violations(case, dict(slots=[127, 5, 0]))), 2)
        self.assertTrue(dmx.violations(case, dict(slots=[128])))

    def test_source_filter_cannot_borrow_other_sender(self):
        case = dict(protocol="sacn", universe=1, source=dict(ip="127.0.0.1", cid="expected"))
        frame = dict(protocol="sacn", universe=1, ip="127.0.0.1", cid="other")
        self.assertFalse(dmx.matches(case, frame))

    def test_preview_termination_start_code_and_settling_do_not_pass(self):
        frame = dmx.decode("sacn", sacn())
        self.assertTrue(dmx.eligible(frame, 1, 0))
        self.assertFalse(dmx.eligible(frame, 0, 1))
        for field, value in (("preview", True), ("terminated", True), ("start_code", 0xdd)):
            self.assertFalse(dmx.eligible(dict(frame, **{field: value}), 1, 0))

    def test_no_packets_or_missing_expected_source_fail(self):
        self.assertEqual(len(dmx.completion_failures(0, {"red": 0}, 3)), 2)
        self.assertEqual(len(dmx.completion_failures(20, {"red": 0}, 3)), 1)
        self.assertEqual(dmx.completion_failures(20, {"red": 3}, 3), [])

    def test_output_rejects_repository_root(self):
        with self.assertRaises(ValueError):
            dmx.output_path(Path(__file__).resolve().parents[2])

    def test_udp_port_validation(self):
        for value in ("1", "16454", "15568", "65535"):
            self.assertEqual(dmx.udp_port(value), int(value))
        for value in ("0", "65536", "-1"):
            with self.assertRaises(argparse.ArgumentTypeError):
                dmx.udp_port(value)
        with self.assertRaises(ValueError):
            dmx.udp_port("invalid")


if __name__ == "__main__":
    unittest.main()
