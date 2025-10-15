import sys

def generate_taskfile(n: int, file_name: str):
    try:
        with open(file_name, "w", encoding="utf-8") as f:
            f.write("version: '3'\n")
            f.write("tasks:\n")

            f.write("  default:\n")
            f.write("    deps: [script_1]\n\n")

            for i in range(1, n + 1):
                script_name = f"script_{i}"
                f.write(f"  {script_name}:\n")
                f.write("    cmds:\n")
                f.write(f"      - echo \"Executing {script_name} ({i}/{n})\"\n\n")

        print(f"File '{file_name}' generated {n} tasks to go-task.")

    except IOError as e:
        print(f"Error writting the file: {e}")

if __name__ == "__main__":
    arg1 = int(sys.argv[1]) if len(sys.argv) > 1 else 1000
    arg2 = sys.argv[2] if len(sys.argv) > 2 else "Taskfile.yml"

    generate_taskfile(arg1, arg2)