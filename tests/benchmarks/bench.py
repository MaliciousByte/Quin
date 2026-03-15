import subprocess
import time
import os

def run_bench(command, name):
    print(f"Running {name}...")
    start = time.time()
    try:
        result = subprocess.run(command, capture_output=True, text=True, check=True)
        end = time.time()
        print(result.stdout.strip())
        return end - start
    except Exception as e:
        print(f"Error running {name}: {e}")
        return None

def main():
    # Define benchmark file paths relative to this script
    script_dir = os.path.dirname(__file__)
    quin_bench = os.path.join(script_dir, "tight_loop.qn")
    py_bench = os.path.join(script_dir, "tight_loop.py")
    js_bench = os.path.join(script_dir, "tight_loop.js")

    # Check if the Quin benchmark file exists
    if not os.path.exists(quin_bench):
        print(f"Error: Quin benchmark file not found at {quin_bench}")
        return

    # Assume 'quin' is the executable. Adjust if it's 'cargo run'
    # For 'cargo run', the path to the .qn file is passed as an argument.
    quin_cmd = ["cargo", "run", "--release", "--", quin_bench]
    py_cmd = ["python", py_bench]
    js_cmd = ["node", js_bench]

    results = {}

    results["Quin"] = run_bench(quin_cmd, "Quin")
    results["Python"] = run_bench(py_cmd, "Python")
    results["JS"] = run_bench(js_cmd, "JS")

    print("\nSummary:")
    for name, duration in results.items():
        if duration is not None:
            print(f"{name}: {duration:.4f}s")

if __name__ == "__main__":
    main()
