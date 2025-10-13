<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../AXES_TOML_GUIDE.md">English</a> •
  <a href="./AXES_TOML_GUIDE.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../AXES_TOML_GUIDE.md), que es la fuente canónica de la documentación.

# Dominando `axes.toml`: La Guía Definitiva

El archivo `axes.toml` es el cerebro de cada uno de tus proyectos. Aquí es donde transformas secuencias de comandos caóticas en flujos de trabajo limpios, reutilizables y potentes. Esta guía es la referencia completa para cada sección y característica que puedes utilizar.

## El Principio Fundamental: Herencia

Antes de sumergirnos en los detalles, recuerda el concepto más importante: **la herencia**.

Cada proyecto `axes` hereda la configuración completa de su proyecto padre. Cuando `axes` ejecuta un comando en el contexto de `my-app/api`, primero lee el `axes.toml` de `my-app/api`, luego, si el objeto a buscar no existe en esa configuración, busca en `my-app`, luego en su padre, y finalmente en el de `global`.

Esto significa que un proyecto hijo puede:

* **Usar** variables y *scripts* definidos en sus padres.
* **Sobrescribir** variables y *scripts* para especializar el comportamiento.

> **Regla de Fusión:** La configuración del hijo siempre tiene precedencia. Si `my-app` define `[vars] version = "1.0"` y `my-app/api` define `[vars] version = "1.1"`, el valor para `api` será `"1.1"`.

### Anatomía de un `axes.toml`

Aquí tienes un ejemplo de un `axes.toml` con todas las secciones principales. Las exploraremos una por una.

```toml
# ==============================================================================
# axes.toml: Guía de Referencia Exhaustiva
# Este archivo sirve como un ejemplo exhaustivo de todas las características disponibles en `axes`.
# ==============================================================================

# --- 1. Metadatos (Opcional) ---
# Proporciona información sobre el proyecto, visible con `axes info`.
version = "2.0.0"
description = "API de Backend para el proyecto WebApp. Proporciona endpoints de datos."

# --- 2. Variables de Entorno ([env]) ---
# Estas variables se inyectan como variables de entorno del sistema en CADA comando ejecutado por `axes` en este contexto.
[env]
# Ideal para secretos (si se definen en un proyecto ancestro exterior) o constantes de entorno.
DATABASE_URL = "postgresql://user:pass@localhost:5432/webapp_db"
LOG_LEVEL = "info"

# --- 3. Variables de `axes` ([vars]) ---
# Variables internas para reutilización dentro de scripts usando la sintaxis `<vars::...>` .
# Promueven la filosofía DRY (Don't Repeat Yourself).
[vars]
image_name = "webapp/api"
# Las variables pueden ser dinámicas, ejecutando un comando en tiempo real.
git_hash = "<run('git rev-parse --short HEAD')>"

# --- 4. Scripts ([scripts]) ---
# El núcleo de `axes`. Define los flujos de trabajo del proyecto.
[scripts]

# Forma simple: un único comando como una cadena de texto.
run = "poetry run uvicorn app.main:app --reload"

# Forma de secuencia: una lista de comandos ejecutados secuencialmente.
# Usa '#' para imprimir mensajes de estado sin invocar el shell.
test = [
    "# Ejecutando tests de la API...",
    "poetry run pytest"
]

# Forma extendida: un diccionario con una descripción (`desc`) y el comando (`run`).
# Esto mejora la salida de `axes info` y `axes run` (sin argumentos).
[scripts.seed_db]
desc = "Rellena la base de datos con datos de prueba."
run = [
  "# Aplicando semillas a la base de datos...",
  # `run` puede contener líneas multiplataforma. `axes` elegirá la correcta.
  # Si la específica del SO no existe, recurre a `default`.
  { windows = "psql.exe -U user -d webapp_db -f ./seed.sql", default = "psql -U user -d webapp_db -f ./seed.sql" }
]

# Script con un parámetro nombrado (`tag`) que tiene un valor por defecto.
[scripts.build]
desc = "Construye la imagen Docker local."
run = "docker build . -t <vars::image_name>:<params::tag(default='latest')>"

# Script que delega el análisis de argumentos al shell usando el prefijo '$'.
# Permite pasar flags y argumentos directamente al comando subyacente.
# Ejemplo de uso: `axes format --check .` se convierte en `poetry run ruff format .`
[scripts.format]
desc = "Formatea el código usando Ruff."
run = "$ poetry run ruff format ."

# Un script complejo que demuestra composición y modificadores de comando.
[scripts.deploy]
desc = "Construye y sube la imagen Docker de la API."
run = [
  "# Paso 1: Construir la imagen (ejecución silenciosa, el comando no se imprime).",
  "@ <scripts::build>", # <-- Composición: llama a otro script de `axes`.

  "# Paso 2: Etiquetar la imagen con el hash del commit (ignora errores si la etiqueta ya existe).",
  "- docker tag <vars::image_name>:latest <vars::image_name>:<vars::git_hash>",

  "# Paso 3: Subir ambas etiquetas en paralelo para máxima velocidad.",
  "> docker push <vars::image_name>:latest", # <-- El prefijo `>` inicia un lote paralelo.
  "> docker push <vars::image_name>:<vars::git_hash>"
]


# --- 5. Opciones de Sesión y Hooks ([options]) ---
[options]

# `at_start`: Se ejecuta una vez al iniciar una sesión con `axes start`.
# Ideal para activar entornos virtuales, iniciar servicios, etc.
at_start = "poetry install --no-root"

# `at_exit`: Se ejecuta al salir de la sesión (con `exit`).
# Ideal para detener servicios, limpiar archivos temporales, etc.
at_exit = "# Saliendo de la sesión de la API..."

# Configuración para el comando `axes open`.
[options.open_with]
# Define "atajos" para abrir el proyecto en diferentes aplicaciones.
# `<path>` es un token especial que se resuelve a la ruta física del proyecto.
editor = "code \"<path>\""
terminal = { windows = "wt -d \"<path>\"", default = "gnome-terminal --working-directory=\"<path>\""}

# `default` especifica qué atajo usar si `axes open` se ejecuta sin argumentos.
default = "editor"
```

---

## 1. Metadatos (Opcional)

Estas claves son puramente informativas y ayudan a documentar tu proyecto.

* `version`: La versión de tu proyecto (ej. `"1.0.0"`). Es accesible en *scripts* a través del token `<version>`.
* `description`: Una breve descripción de lo que hace el proyecto. Se muestra en comandos como `info`.

```toml
version = "2.1.0-beta"
description = "Servicio principal de autenticación."
```

---

## 2. Variables de Interpolación `[vars]`

La sección `[vars]` es tu mejor herramienta para seguir el principio **DRY (Don't Repeat Yourself)**. Define valores aquí una vez y reutilízalos en múltiples *scripts*.

**Definición:**

```toml
[vars]
output_dir = "build/release"
compiler_flags = "--optimization-level 3 -DNDEBUG"
```

**Uso:**
Para usar una variable, utiliza la sintaxis `<vars::nombre_variable>`. `axes` reemplazará el token con el valor de la variable antes de ejecutar el comando.

```toml
[scripts]
# Usa las variables definidas arriba.
build = "c++ <vars::compiler_flags> -o <vars::output_dir>/app main.cpp"
```

Las variables también pueden componerse entre sí y usar otros *tokens* de `axes`:

```toml
[vars]
# El directorio de artefactos depende del nombre del proyecto.
artifact_dir = "artifacts/<name>"
# El nombre del archivo final se compone de otra variable.
final_zip = "<vars::artifact_dir>/<name>.zip"
```

## 3. Scripts y Flujos de Trabajo `[scripts]`

Esta es la sección principal de `axes`. Un "*script*" es una entrada con nombre para una tarea que deseas realizar. Cada clave en la tabla `[scripts]` define un comando que puedes ejecutar con `axes <nombre_del_script>`.

### 3.1. Sintaxis de Comandos

Puedes definir un comando de varias maneras, desde la más simple hasta la más completa.

#### **A. Comando Simple (String)**

La forma más básica. `axes` la tratará como el comando por defecto para tu sistema operativo actual.

```toml
[scripts]
check = "cargo check"
serve = "python -m http.server 8000"
```

#### **B. Secuencia de Comandos (Array de Strings)**

Para flujos de trabajo que requieren múltiples pasos, define el *script* como una lista de cadenas. `axes` ejecutará cada comando en orden y se detendrá si alguno falla (a menos que uses un modificador de ejecución).

```toml
[scripts]
deploy = [
    "echo 'Construyendo la aplicación...'",
    "npm run build",
    "echo 'Desplegando en el servidor...'",
    "scp -r ./dist/* user@server:/var/www/my-app",
]
```

#### **C. Estructura Extendida (Tabla)**

Para añadir una descripción o definir comportamiento multiplataforma, usa una tabla TOML.

* **Con Descripción:**

    ```toml
    [scripts]
    lint = { desc = "Ejecuta el linter para encontrar problemas de estilo.", run = "eslint ." }
    test = { desc = "Ejecuta la suite de pruebas completa.", run = ["npm run test:unit", "npm run test:e2e"] }
    ```

    La clave `desc` se mostrará en comandos como `axes <ctx> info`.

* **Multiplataforma:**
    Define un único *script* que se comporte de manera diferente según el sistema operativo. `axes` seleccionará automáticamente el comando correcto.

    ```toml
    [scripts.browse]
    desc = "Abre la documentación local en el navegador por defecto."
    windows = "start http://localhost:8080"
    macos = "open http://localhost:8080"
    linux = "xdg-open http://localhost:8080"
    default = "echo 'Visita http://localhost:8080 en tu navegador.'"
    ```

### **3.2. Modificadores de Ejecución (Prefijos)**

Puedes controlar cómo se ejecuta cada línea en una secuencia utilizando prefijos especiales. Se pueden combinar (ej. `>- @ my_command`).

> **Regla Clave:** Los modificadores solo afectan a la línea donde están escritos. **No se "heredan"** cuando un *script* es compuesto por otro. El control de ejecución siempre pertenece al *script* "llamador".

| Prefijo | Nombre                  | Descripción                                                                                                   |
| :------ | :---------------------- | :------------------------------------------------------------------------------------------------------------ |
| `-`     | **Ignorar Errores**     | `axes` continuará con el siguiente comando en una secuencia aunque este falle (salga con código distinto de cero). |
| `>`     | **Ejecución Paralela**  | `axes` lanza este comando y continúa inmediatamente con el siguiente, sin esperar a que termine.                 |
| `@`     | **Modo Silencioso**     | `axes` no imprimirá el comando (`→ my_command`) antes de ejecutarlo. Útil para una salida limpia.              |
| `#`     | **Modo Eco**            | Toda la línea se trata como una cadena a imprimir en la consola, no como un comando a ejecutar.                 |
| `\|`    | **Terminador**          | Indica explícitamente al *parser* de prefijos que se detenga. Útil para comandos que comienzan con un carácter especial. |

#### **Ejemplos de Modificadores**

**Ignorar Errores (`-`):**

```toml
[scripts]
# Intenta limpiar la caché, pero no falla si el directorio no existe.
build = [
    "-rm -rf .cache",
    "npm run build"
]
```

**Ejecución Paralela (`>`):**

```toml
[scripts]
# Inicia el backend y el frontend simultáneamente.
dev = [
    "> axes api dev",
    "> axes frontend dev"
]
```

**Modo Silencioso y Eco (`@`, `#`):**

```toml
[scripts]
setup = [
    "# --- Configurando entorno ---", # Esta línea se imprimirá.
    "@source ./.env",                  # Este comando se ejecutará, pero no se mostrará.
    "# Entorno listo."
]
```

**Terminador (`|`):**

```toml
[scripts]
# El `-v` es un flag para `my_tool`, no un modificador para `axes`.
advanced = ">| -v --some-flag"
```

### 3.3. Composición de Scripts: El Corazón de la Reutilización

Una de las características más potentes de `axes` es su capacidad para construir *scripts* complejos a partir de piezas más pequeñas reutilizables mediante la expansión de *tokens* **antes** de la ejecución.

* **Sintaxis:** `<scripts::nombre_otro_script>`

Cuando `axes` prepara tus *scripts*, **compone estructuralmente** los comandos. Si llamas a un *script* con múltiples líneas, sus comandos se insertan directamente en la lista de comandos del padre.

**Ejemplo de un Flujo de Calidad de Código:**

```toml
# en `my-app/.axes/axes.toml` (el padre)
[scripts]
# Scripts base reutilizables
lint = { desc = "Ejecuta el linter.", run = "ruff check ." }
test = { desc = "Ejecuta la suite de pruebas.", run = ["pytest tests/unit", "pytest tests/integration"] }

# Script compuesto que une los anteriores.
# El control de ejecución (secuencial, paralelo) pertenece a `quality`.
quality = [
    "# Ejecutando todas las comprobaciones de calidad...",
    "<scripts::lint>",
    "> <scripts::test>", # `test` en sí es secuencial, pero `quality` lo ejecuta en paralelo.
]
```

Ejecutar `axes quality` ejecutará `ruff check .`, y una vez que termine, lanzará ambos comandos `pytest` en paralelo.

## 4. El Motor de Expansión: Potenciando Tus Scripts

La característica que une todo es su motor de expansión de *tokens*. Cualquier valor de cadena en tu `axes.toml` puede contener *tokens* especiales en el formato `<...>` que `axes` procesará.

La expansión ocurre de forma perezosa (lazy) y sus resultados se guardan como un Árbol de Sintaxis Abstracta (AST) puro en una caché binaria (`.axes/config.cache.bin`), haciendo que las ejecuciones posteriores sean extremadamente rápidas.

### 4.1. Tokens de Valor Estático

Estos *tokens* se resuelven a sus valores finales durante la fase de expansión (compilación JIT).

#### **Tokens de Metadatos del Proyecto**

| Token             | Valor de Expansión                                                  |
| :---------------- | :------------------------------------------------------------------ |
| `<name>`    | El nombre cualificado completo del proyecto (ej. `my-app/api`).        |
| `<path>`    | La ruta física absoluta al directorio raíz del proyecto.             |
| `<uuid>`    | El identificador único universal del proyecto.                        |
| `<version>` | La versión definida en el `axes.toml` del proyecto.                   |

#### **Tokens de Variables**

* **`<vars::nombre_variable>`:** Se expande al valor de la variable definida en la sección `[vars]`.

**Ejemplo Combinado:**

```toml
# en el `axes.toml` padre de `my-app`
[vars]
docker_registry = "registry.example.com/my-org"

# en el `axes.toml` hijo de `my-app/api`
[scripts]
docker_build = "docker build -t <vars::docker_registry>/<name>:<version> ."
```

### 4.2. Token de Ejecución Dinámica: `<run::(...)>`

A veces, necesitas el **resultado** de un comando para usarlo en otro.

* **Sintaxis:** `<run('comando_a_ejecutar')>`
* **Comportamiento:** `axes` ejecuta `command_to_execute` **en tiempo de ejecución**, captura su salida estándar (stdout), la limpia (eliminando espacios en blanco finales) e inyecta el resultado en el comando principal.

> **Importante:** La salida de los *tokens* `run` **nunca** se almacena en caché para garantizar que los datos sean siempre frescos.

**Ejemplo: Etiquetado de Docker con Hash de Git:**

```toml
[scripts]
tag_release = "docker tag my-app:latest my-app:<run('git rev-parse --short HEAD')>"
```

Al ejecutar `axes tag_release`:

1. `axes` se prepara para ejecutar el *script* `tag_release`.
2. Encuentra el *token* `<run::(...)>`.
3. Ejecuta `git rev-parse --short HEAD`.
4. La salida de git (ej. `a1b2c3d`) es capturada.
5. El comando final se ensambla como `docker tag my-app:latest my-app:a1b2c3d` y luego se ejecuta.

### 4.3. Tokens de Parámetros en Tiempo de Ejecución: `<params::...>`

Esta familia de *tokens* especiales no se expande por adelantado. Son marcadores de posición que se resuelven en el último momento por el `task_executor`, utilizando los argumentos que proporcionas en la línea de comandos.

(Esto se cubre en detalle en la siguiente sección).

## 5. Scripts como Funciones: El Sistema de Parámetros (`<params::...`)

`axes` no solo ejecuta *scripts*; te permite definir verdaderas "funciones" de línea de comandos que aceptan argumentos de manera estructurada.

Esto elimina la necesidad de escribir *scripts* `bash` complejos para analizar flags y parámetros.

Todos la lógica de parámetros se controla a través del *namespace* `<params::...>` y sigue un **paradigma declarativo**: defines los parámetros que espera tu *script*, y `axes` valida la entrada del usuario **antes** de ejecutar nada.

> **Regla de Oro:** Si pasas argumentos a un *script* desde la línea de comandos (`axes mi-script arg1 --flag`), el `axes.toml` de ese *script* **debe** usar *tokens* `<params::...>` para consumirlos. Si quedan argumentos sin consumir por ningún token (y no hay un token genérico `<params>`), `axes` devolverá un error para prevenir un comportamiento inesperado.

### 5.1. Parámetros Posicionales

Estos son argumentos pasados sin un *flag*. Se accede a ellos por su índice (comenzando en `0`).

* **Sintaxis Básica:** `<params::0>`, `<params::1>`, etc.
* **Comportamiento:** Se reemplazan por el argumento posicional en ese índice. Si el argumento no existe y no es `required` ni tiene un `default`, se reemplaza por una cadena vacía.

#### **Modificadores para Posicionales `(...)`**

* `required`: La ejecución falla si no se proporciona el argumento.
* `default='value'`: Proporciona un valor por defecto si el argumento no se pasa en la CLI.
* `map='--new-flag'`: Transforma el argumento posicional en un flag con valor. Si se escribe `my-value`, el *token* se expande a `"--new-flag my-value"`.

**Ejemplo: Un *script* simplificado de `git commit`.**

```toml
[scripts]
# Acepta el mensaje de commit como el primer argumento posicional requerido.
commit = "git commit -m \"<params::0(required)>\""
```

**Ejecución:**

```sh
# El '0' se refiere a "Fix: ..."
axes commit "Fix: Fix authentication bug"

# Comando ejecutado:
# git commit -m "Fix: Fix authentication bug"

# Falla si no se proporciona:
axes commit
# -> Error: El argumento posicional en el índice 0 es requerido pero no fue proporcionado.
```

### 5.2. Parámetros Nombrados (Flags)

También puedes hacer que tus *scripts* reaccionen a *flags* (`--nombre`) pasados desde la CLI.

* **Sintaxis Básica:** `<params::nombre-del-flag>`
* **Comportamiento por Defecto (Pase-a-través):** El *token* busca el *flag* en la CLI y lo reinjecta tal cual, junto con su valor si lo tiene. Si no se encuentra, se expande a una cadena vacía.

#### **Modificadores para Flags `(...)`**

* `required`: Falla si el *flag* (o su alias) no está presente.
* `default='value'`: Si el *flag* **no se proporciona en absoluto**, se usará este `default`. También se aplica si el *flag* se proporciona **sin valor** (ej. `command --my-flag`).
* `alias='-a'`: Permite que el *flag* sea reconocido por un alias corto. `axes` lanzará un error si el usuario proporciona tanto el nombre completo como el alias.
* `map='--new-name'`: Reemplaza el nombre del *flag* en la salida. Muy útil para abstraer herramientas subyacentes.
* `map=' '`: Un caso especial. Indica que solo quieres inyectar el **valor** del *flag*, no el nombre del *flag* en sí. Ideal para inyectar valores en posiciones donde no se espera un *flag*.

**Ejemplo: Script `build` con modo `release` (Pase-a-través Simple):**

```toml
# axes.toml
[scripts]
build = "cargo build <params::release>"
```

```sh
axes . build            # -> cargo build
axes . build --release  # -> cargo build --release
axes . build --another-param  # -> Error: Se proporcionaron argumentos inesperados. El script no define un token `<params>` genérico para aceptarlos.
#Argumentos no manejados proporcionados: --another-param
```

**Script `test` con alias:**

```toml
# axes.toml
[scripts]
test = "pytest <params::marker(alias='-m')>"
```

```sh
axes . test --marker slow   # -> pytest --marker slow
axes . test -m smoke        # -> pytest --marker smoke
axes . test -m smoke --marker slow # -> Error: Conflicto: Se proporcionaron tanto el flag '--marker' como su alias '-m'.
```

**Otro caso de uso posible, pero no recomendado porque generará conflictos:**

```toml
# axes.toml
[scripts]
copy-file = "rsync <params::files-from(alias='from', default='list.txt')> <params::copy-in(alias='in', required)>"
```

```sh
axes . copy-file from file.txt in ./backup            # -> rsync --files-from file.txt --copy-in ./backup
axes . copy-file in ./backup                          # -> rsync --files-from list.txt --copy-in ./backup
axes . copy-file in ./backup --copy-in /another/place # -> Error: Conflicto: Se proporcionaron tanto el flag '--copy-in' como su alias 'in'.
```

**Script `deploy` con `map` y `required`:**

```toml
# axes.toml
[scripts]
# El script interno espera --environment, pero exponemos --env al usuario.
deploy = "terraform apply <params::env(map='--environment', required)>"
```

```sh
axes . deploy --env staging      # -> terraform apply --environment staging
axes . deploy                    # -> Error: El flag '--env' es requerido pero no fue proporcionado.
```

**Script `docker` con `map=' '` para extracción de valor:**
Este es un patrón avanzado para inyectar valores en lugares donde un flag no es válido.

```toml
# axes.toml
[scripts]
# La etiqueta de la imagen se pasa como un flag pero se inyecta como un valor posicional.
docker_tag = "docker tag my-image:latest my-org/my-image:<params::tag(map='', default='latest')>"
```

```sh
# Ejecución 1: Usa el valor por defecto
axes docker_tag
# Comando ejecutado: `docker tag my-image:latest my-org/my-image:latest`

# Ejecución 2: Especifica la etiqueta
axes docker_tag --tag v1.2.0
# Comando ejecutado: `docker tag my-image:latest my-org/my-image:v1.2.0`
```

### 5.3. El Colector Genérico: `<params>`

Este es el *token* "colector". Es útil cuando quieres pasar un número variable de argumentos o *flags* a un comando subyacente sin tener que definirlos todos explícitamente.

* **Sintaxis:** `<params>`
* **Comportamiento:** Se reemplaza por **todos los argumentos** (posicionales y nombrados) que **no fueron consumidos** por un *token* explícito (`::0`, `::flag`, etc.), manteniendo su orden original.

**Ejemplo: Un `wrapper` genérico para `npm install`.**

```toml
[scripts]
# `add` pasa todos los argumentos restantes a `npm install`,
# pero también maneja explícitamente un flag `--save-dev` con un alias `-D`.
add = "npm install <params::save-dev(alias='-D')> <params>"
```

**Ejecución:**

```sh
# Instala una dependencia normal
axes add react
# Comando ejecutado: `npm install react`

# Instala una dependencia de desarrollo
axes add -D typescript
# `-D` es consumido por <...::save-dev> y se expande a `--save-dev`.
# `typescript` no es consumido y es recogido por <params>.
# Comando ejecutado: `npm install --save-dev typescript`

# Instala múltiples dependencias con flags adicionales
axes add react react-dom --force
# Comando ejecutado: `npm install react react-dom --force`
```

Al combinar estos patrones, puedes construir interfaces de línea de comandos increíblemente ricas y robustas para tus *scripts*, todo dentro de la sencillez de `axes.toml`.

> Para una guía completa con ejemplos detallados de cada tipo de parámetro y modificador, por favor consulta la **[Guía del Sistema de Argumentos (`ARG_PARSER.md`)](./ARG_PARSER.md)**.

## 6. Opciones de Entorno y Hooks

### 6.1. Variables de Entorno `[env]`

Cualquier par clave-valor en `[env]` se inyecta como una variable de entorno en el subproceso del *script*. Se heredan y pueden ser sobrescritos.

```toml
[env]
DATABASE_URL = "postgres://user:pass@localhost/db"
APP_ENV = "development"
```

### 6.2. Opciones de Sesión y Herramientas `[options]`

#### **Hooks de Sesión: `at_start` y `at_exit`**

Son *scripts* completos que se ejecutan automáticamente al entrar y salir de una sesión interactiva (`axes <ctx> start`). Pueden aceptar parámetros pasados al comando `start`.

**Ejemplo:**

```toml
[options]
at_start = { desc = "Activa venv y levanta la DB.", run = [
    "source .venv/bin/activate",
    "docker-compose up -d <params::service(default='db')>"
]}
at_exit = "docker-compose down"
```

#### **Configuración del Comando `open`: `[options.open_with]`**

Define atajos para el comando `axes <ctx> open`. Cada entrada es un *script* completo y puede aceptar parámetros.

**Ejemplo:**

```toml
[options.open_with]
edit = { desc = "Abre el proyecto en VS Code.", run = "code \"<path>\"" }
terminal = "wt -d \"<path>/<params::0(default='.')>\"" # Terminal de Windows en subcarpeta
default = "edit"
```

---

## Conclusión

Ahora tienes una descripción completa del archivo `axes.toml`. Al combinar estas características, puedes construir flujos de trabajo potentes, portátiles y auto-documentados que potenciarán tu productividad de desarrollo.

➡️ **Lectura Recomendada Siguiente: [Referencia Completa de Comandos (`COMMANDS.md`)](./COMMANDS.md)**