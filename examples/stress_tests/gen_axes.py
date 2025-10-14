import sys

BASE_HEADER = """
# === Development Scripts ===
[scripts]

"""

def generate_axes_toml(n:int, file_name:str):
    with open(file_name, "w", encoding="utf-8") as f:
        f.write(BASE_HEADER)
        for i in range(1, n+1):
            f.write(f"script_{i} = \"@echo Script number {i} executed...\"\n")
        f.write("\n")
    print(f"Archive '{file_name}' generated with {n} scripts.")

if __name__ == "__main__":
    arg1 = int(sys.argv[1]) if len(sys.argv) > 1 else 1000
    arg2 = sys.argv[2] if len(sys.argv) > 2 else "axes.toml"

    generate_axes_toml(arg1, arg2)