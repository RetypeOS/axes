import sys

def generate_makefile(n:int, file_name:str):
    try:
        with open(file_name, "w", encoding="utf-8") as f:
            f.write(".PHONY: default\n")
            f.write("default: receta_1\n\n")

            for i in range(1, n + 1):
                script_name = f"script_{i}"
                
                f.write(f".PHONY: {script_name}\n")
                f.write(f"{script_name}:\n")
                f.write(f"\t@echo \"Executing {script_name} ({i}/{n})\"\n\n")

        print(f"Archive '{file_name}' generated with {n} scripts.")

    except IOError as e:
        print(f"❌ Error al escribir en el archivo: {e}")

if __name__ == "__main__":
    arg1 = int(sys.argv[1]) if len(sys.argv) > 1 else 1000
    arg2 = sys.argv[2] if len(sys.argv) > 2 else "makefile.mk"

    generate_makefile(arg1, arg2)
