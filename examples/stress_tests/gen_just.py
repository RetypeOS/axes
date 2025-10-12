# generar_justfile.py
# Script para crear un justfile con N recetas "basura"

def generar_justfile(n, nombre_archivo="justfile"):
    with open(nombre_archivo, "w", encoding="utf-8") as f:
        f.write('# justfile generado automáticamente\nset shell := ["cmd.exe", "/C"]\n')
        for i in range(1, n + 1):
            f.write(f"receta_{i:04d}:\n")
            f.write(f"    echo \"Comando basura {i:04d}\"\n\n")
    print(f"Se generó '{nombre_archivo}' con {n} recetas.")

if __name__ == "__main__":
    # Cambia este número para generar más o menos recetas
    generar_justfile(100000)