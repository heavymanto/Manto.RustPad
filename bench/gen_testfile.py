import sys, time
out = sys.argv[1]; reps = int(sys.argv[2])
pay = ("lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore")[:90]
assert len(pay) == 90
block = "".join(f"L{i:07d} {pay}\n" for i in range(1_000_000)).encode()
assert len(block) == 100_000_000
t = time.time()
with open(out, "wb") as f:
    for _ in range(reps): f.write(block)
print(out, reps * len(block), "byte", round(time.time() - t, 1), "s")
