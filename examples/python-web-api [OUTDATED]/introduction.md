
# Flujo de Uso del Ejemplo de python-web-api

Este ejemplo demuestra de forma práctica y completa cómo `axes` simplifica y estandariza el flujo de trabajo de un proyecto Python.

Una vez esté preparado siga estos pasos:

1. **Registrar el Proyecto (si no se usó `init`):**

    ```sh
    cd examples/python-web-api
    axes register . --parent global  # O el padre que corresponda
    ```

2. **Configuración Inicial (un solo comando):**

    ```sh
    axes python-web-api setup
    ```

    Esto creará el directorio `.venv` e instalará `Flask`, `pytest` y `flake8`.

3. **Iniciar una Sesión de Desarrollo:**

    ```sh
    axes python-web-api start
    ```

    * `axes` ejecutará `source ./.venv/bin/activate` silenciosamente.
    * El usuario aterrizará en un prompt de shell con el entorno virtual ya activado.

4. **Trabajar dentro de la Sesión:**

    ```sh
    # Iniciar el servidor de desarrollo. `flask` está en el PATH gracias a `at_start`.
    axes dev

    # En otra terminal (dentro de otra sesión de `axes start`):
    # Ejecutar la suite de calidad completa con un solo comando.
    axes check
    ```

5. **Abrir el Proyecto en VS Code:**

    ```sh
    axes python-web-api open
    ```

    `axes` ejecutará `code .` en la raíz del proyecto.
