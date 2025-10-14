import sys

def generate_justfile(n:int, file_name:str):
    with open(file_name, "w", encoding="utf-8") as f:
        f.write('# --- justfile generated automatically ---\nset shell := ["cmd.exe", "/C"]\n')
        for i in range(1, n + 1):
            f.write(f"script_{i}:\n")
            f.write(f"    echo \"Command executed {i}\"\n\n")
    print(f"Archive '{file_name}' generated with {n} scripts.")

if __name__ == "__main__":
    arg1 = int(sys.argv[1]) if len(sys.argv) > 1 else 1000
    arg2 = sys.argv[2] if len(sys.argv) > 2 else "justfile"

    generate_justfile(arg1, arg2)