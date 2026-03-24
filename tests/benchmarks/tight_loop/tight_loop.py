import time

def tight_loop():
    i = 0
    sum = 0
    while i < 10000000:
        sum = sum + i
        i = i + 1
    return sum

start = time.time()
print(tight_loop())
end = time.time()
print(f"Python Time: {end - start:.4f}s")
