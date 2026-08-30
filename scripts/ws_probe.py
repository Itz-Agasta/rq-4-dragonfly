#!/usr/bin/env python3
"""Read the dragonfly-core telemetry socket and print decoded frames.

The D6 acceptance check. Confirms three things a `candump` cannot: that the
frames reassemble, that they decode, and that every channel is populated.

    just probe        # 20 frames
    just probe 200
"""

import asyncio
import sys

import msgpack
import websockets

URL = "ws://127.0.0.1:8787/ws"

# The twenty measured channels. Per-cylinder ones are arrays here and are
# flattened to cht_1_k .. cht_4_k when the Parquet is written.
SCALARS = [
    "rpm", "map_pa", "mat_k", "boost_pa", "maf_kgs", "fuel_flow_kgh", "lambda",
    "oil_p_pa", "oil_t_k", "coolant_t_k", "tc_rpm", "bus_v", "vib_rms_g",
    "vib_kurtosis",
]
ARRAYS = ["cht_k", "egt_k", "lambda_k"]


def missing(frame):
    """Channels that are absent or not a finite number."""
    out = []
    for key in SCALARS:
        v = frame.get(key)
        if v is None or v != v:
            out.append(key)
    for key in ARRAYS:
        values = frame.get(key) or []
        if len(values) != 4 or any(v != v for v in values):
            out.append(key)
    return out


async def main(count):
    async with websockets.connect(URL) as socket:
        first = last = None
        gaps = 0
        holes = set()
        for _ in range(count):
            frame = msgpack.unpackb(await socket.recv(), raw=False)
            if first is None:
                first = frame["seq"]
            elif frame["seq"] != last + 1:
                gaps += 1
            last = frame["seq"]
            holes.update(missing(frame))
            print(
                "seq %-6d t %7.2f  link %-5s  %5.0f rpm  %6.0f hPa  "
                "EGT %4.0f %4.0f %4.0f %4.0f K  turbo %6.0f  %4.1f V  %4.2f g"
                % (
                    frame["seq"], frame["t_s"], frame["link_ok"],
                    frame["rpm"], frame["map_pa"] / 100.0,
                    *frame["egt_k"], frame["tc_rpm"], frame["bus_v"],
                    frame["vib_rms_g"],
                )
            )
        print("\n%d frames, seq %d..%d, %d gap(s)" % (count, first, last, gaps))
        print("channels with no data: %s" % (sorted(holes) or "none"))
        return 1 if holes or gaps else 0


if __name__ == "__main__":
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 20
    sys.exit(asyncio.run(main(n)))
