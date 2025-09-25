# Dominando el `axes.toml`: La Guía Definitiva

El archivo `axes.toml` es el cerebro de cada uno de tus proyectos. Es aquí donde transformas secuencias de comandos caóticas en flujos de trabajo limpios, reutilizables y potentes. Esta guía es la referencia completa de cada sección y característica que puedes usar.

## El Principio Fundamental: Herencia

Antes de sumergirnos en los detalles, recuerda el concepto más importante: **la herencia**.

Cada proyecto `axes` hereda la configuración completa de su proyecto padre. Cuando `axes` ejecuta un comando en el contexto de `mi-app/api`, primero lee el `axes.toml` de `mi-app/api`, y luego "fusiona" la configuración de `mi-app` por debajo, y finalmente la de `global`.

Esto significa que un proyecto hijo puede:

* **Usar** variables y scripts definidos en sus padres.
* **Sobrescribir** variables y scripts para especializar el comportamiento.

> **Regla de Fusión:** La configuración del hijo siempre tiene prioridad. Si `mi-app` define `[vars] version = "1.0"` y `mi-app/api` define `[vars] version = "1.1"`, el valor para `api` será `1.1`.

### Anatomía de un `axes.toml`

Aquí tienes un ejemplo de un `axes.toml` con todas las secciones principales. Las exploraremos una por una.

```toml
# --- Metadatos (Opcional) ---
version = "1.0.0"
description = "Un proyecto de ejemplo."

# --- Variables de Entorno para cada ejecución ---
[env]
NODE_ENV = "development"

# --- Variables para reutilizar en scripts ---
[vars]
dist_dir = "dist/"

# --- Scripts y Flujos de Trabajo ---
[scripts]
build = "npm run build -- --output <axes::vars::dist_dir>"
serve = "npm run serve"

# --- Opciones y Hooks ---
[options]
# Se ejecuta al iniciar una sesión con `axes . start`
at_start = "nvm use 18"
# Se ejecuta al salir de la sesión
at_exit = "echo 'Limpiando sesión...'"

# Configuración para el comando `axes . open`
[options.open_with]
editor = "code \"<axes::path>\""
default = "editor"
```

---

## 1. Metadatos (Opcional)

Estas claves son puramente informativas y ayudan a documentar tu proyecto.

* `version`: La versión de tu proyecto (ej. `"1.0.0"`). Es accesible en los scripts a través del token `<axes::version>`.
* `description`: Una breve descripción de lo que hace el proyecto. Se muestra en comandos como `info`.

```toml
version = "2.1.0-beta"
description = "El servicio de autenticación principal."
```

---

## 2. Variables de Interpolación `[vars]`

La sección `[vars]` es tu mejor herramienta para seguir el principio **DRY (Don't Repeat Yourself)**. Define valores aquí una vez y reutilízalos en múltiples scripts.

**Definición:**

```toml
[vars]
output_dir = "build/release"
compiler_flags = "--optimization-level 3 -DNDEBUG"
```

**Uso:**
Para usar una variable, utiliza la sintaxis `<axes::vars::nombre_variable>`. `axes` reemplazará el token con el valor de la variable antes de ejecutar el comando.

```toml
[scripts]
# Usa las variables definidas arriba.
build = "c++ <axes::vars::compiler_flags> -o <axes::vars::output_dir>/app main.cpp"
```

Las variables también pueden componerse entre sí y usar otros tokens de `axes`:

```toml
[vars]
# El directorio de artefactos depende del nombre del proyecto.
artifact_dir = "artifacts/<axes::name>"
# El nombre del archivo final se compone de otra variable.
final_zip = "<axes::vars::artifact_dir>/<axes::name>.zip"
```

## 3. Scripts y Flujos de Trabajo `[scripts]`

Esta es la sección principal de `axes`. Un "script" es un punto de entrada con nombre para una tarea que quieres realizar, desde una simple compilación hasta una compleja orquestación de despliegue.

### 3.1. Scripts Simples

La forma más básica de un script es una única cadena de comando.

**Sintaxis:**

```toml
[scripts]
nombre_script = "comando a ejecutar"
```

**Ejemplo:**

```toml
[scripts]
# Comprueba el código en busca de errores sin compilar.
check = "cargo check"

# Inicia un servidor de desarrollo simple.
serve = "python -m http.server 8000"
```

**Ejecución:**

```sh
axes . check
axes . serve
```

### 3.2. Scripts Secuenciales

Para flujos de trabajo que requieren múltiples pasos en un orden específico, puedes definir un script como una lista de cadenas. `axes` ejecutará cada comando en orden y se detendrá si alguno de ellos falla.

**Sintaxis:**

```toml
[scripts]
nombre_script = [
    "comando 1",
    "comando 2",
    "comando 3",
]
```

**Ejemplo:**

```toml
[scripts]
# Un flujo completo de construcción y despliegue para una aplicación web estática.
deploy = [
    "echo 'Limpiando compilaciones anteriores...'",
    "rm -rf ./dist",
    "echo 'Construyendo la aplicación...'",
    "npm run build",
    "echo 'Desplegando en el servidor...'",
    "scp -r ./dist/* user@server:/var/www/my-app",
    "echo '🚀 Despliegue completado!'"
]
```

**Ejecución:**

```sh
axes . deploy
```

### 3.3. Estructura Extendida: Añadiendo Descripciones

Para que tus scripts sean más fáciles de entender para tu equipo (¡y para tu yo del futuro!), puedes usar una tabla TOML para añadir una descripción. La descripción se mostrará en comandos como `axes . info`.

**Sintaxis:**

```toml
[scripts]
nombre_script = { desc = "Descripción del script.", run = "comando" }
# O para secuencias:
nombre_script_secuencia = { desc = "Descripción.", run = ["comando 1", "comando 2"] }
```

**Ejemplo:**

```toml
[scripts]
lint = { desc = "Ejecuta el linter para encontrar problemas de estilo en el código.", run = "eslint ." }
test = { desc = "Ejecuta la suite de tests completa.", run = ["npm run test:unit", "npm run test:e2e"] }
```

### 3.4. Scripts Multiplataforma

A menudo, un mismo concepto (como "abrir el explorador de archivos") requiere comandos diferentes en Windows, macOS y Linux. `axes` te permite definir un único script que se comporta de forma diferente según el sistema operativo.

**Sintaxis:**

```toml
[scripts.nombre_script]
desc = "Descripción opcional."
windows = "comando_para_windows"
macos = "comando_para_macos"
linux = "comando_para_linux"
# `default` se usa si el SO actual no coincide con ninguno de los anteriores.
default = "comando_generico"
```

Cada clave de sistema operativo (`windows`, `macos`, etc.) puede ser una cadena simple o una secuencia de comandos.

**Ejemplo:**

```toml
[vars]

[scripts.browse]
desc = "Abre la documentación local en el navegador por defecto."
windows = "start http://localhost:8080"
macos = "open http://localhost:8080"
linux = "xdg-open http://localhost:8080"
default = "echo 'Visita http://localhost:8080 en tu navegador.'"
```

Ahora, cualquier miembro del equipo puede ejecutar `axes . browse` y obtendrá el comportamiento correcto para su sistema.

### 3.5. Modificadores de Ejecución

Puedes controlar cómo se ejecuta cada línea de un script usando prefijos especiales.

#### **Ejecución Paralela con `>`**

Si prefijas un comando con `>` en una secuencia, `axes` le dice: "lanza este comando, pero no esperes a que termine antes de lanzar el siguiente". Esto es ideal para iniciar procesos de larga duración como servidores de desarrollo.

**Ejemplo:**

```toml
[scripts]
# Inicia los servidores de backend y frontend simultáneamente.
dev = [
    "> axes api dev",
    "> axes frontend dev"
]
```

Al ejecutar `axes . dev`, `axes` lanzará el script `dev` de `api` y, inmediatamente después, el script `dev` de `frontend`, sin esperar a que el primero termine.

#### **Ignorar Errores con `-`**

Normalmente, si un comando en una secuencia falla, toda la secuencia se detiene. A veces, quieres que un comando se ejecute pero no te importa si falla. Prefija ese comando con `-` para que `axes` ignore su código de salida.

**Ejemplo:**

```toml
[scripts]
# Intenta limpiar la caché, pero no falles si el directorio no existe.
build = [
    "-rm -rf .cache",
    "npm run build"
]
```

Aquí, si `rm` falla (por ejemplo, porque `.cache` no existía), `axes` continuará y ejecutará `npm run build`. Esto es muy útil para tareas de limpieza opcionales.

## 4. El Motor de Expansión: Dando Superpoderes a tus Scripts

La característica que une todo en `axes` es su motor de expansión de tokens. Cualquier valor de cadena en tu `axes.toml` (en scripts, variables, etc.) puede contener tokens especiales con el formato `<axes::...>` que `axes` procesará antes de ejecutar el comando.

Este sistema te permite crear flujos de trabajo dinámicos, componibles y conscientes del contexto.

### 4.1. Tokens de Metadatos del Proyecto

Estos tokens te dan acceso a la información intrínseca del proyecto en cuyo contexto se está ejecutando el script.

| Token             | Valor de Expansión                                        | Ejemplo de Uso                                                                   |
| :---------------- | :-------------------------------------------------------- | :------------------------------------------------------------------------------- |
| `<axes::name>`    | El nombre cualificado completo del proyecto.              | `echo 'Construyendo <axes::name>...'` -> `Construyendo mi-app/api...`              |
| `<axes::path>`    | La ruta física (absoluta y limpia) al directorio raíz del proyecto. | `docker build -t app . -f <axes::path>/Dockerfile`                               |
| `<axes::uuid>`    | El identificador único universal del proyecto.            | `aws s3 cp ... s3://bucket/<axes::uuid>/`                                        |
| `<axes::version>` | La versión definida en el `axes.toml` del proyecto.       | `echo 'Desplegando versión <axes::version>'` -> `Desplegando versión 1.2.0-beta` |

### 4.2. Tokens de Variables

Estos tokens te permiten inyectar los valores que has definido en las secciones `[vars]` y `[env]`.

* **`<axes::vars::nombre_variable>`:** Se expande al valor de la variable definida en la sección `[vars]`. `axes` buscará la variable en el `axes.toml` del proyecto actual y luego subirá por el árbol de herencia hasta que la encuentre.
* **`<axes::env::NOMBRE_VARIABLE>`:** Se expande al valor de la variable definida en `[env]`. Funciona igual que las `vars` a nivel de herencia.

**Ejemplo Combinado:**

```toml
# en el `axes.toml` del padre `mi-app`
[vars]
docker_registry = "registry.example.com/mi-org"

# en el `axes.toml` del hijo `mi-app/api`
[scripts]
# Construye y etiqueta una imagen Docker con el nombre del proyecto y el registro del padre.
docker_build = "docker build -t <axes::vars::docker_registry>/<axes::name>:<axes::version> ."
```

### 4.3. Composición de Scripts: El Corazón de la Reutilización

Esta es una de las características más potentes. Puedes construir scripts complejos a partir de piezas más pequeñas y reutilizables, incluso si esas piezas están definidas en proyectos padres.

* **`<axes::scripts::nombre_otro_script>`:** `axes` reemplazará este token con el **contenido** del script `nombre_otro_script` y luego expandirá cualquier token que *ese* contenido pueda tener.

**Ejemplo de un Flujo de Calidad de Código:**

```toml
# en `mi-app/.axes/axes.toml` (el padre)
[vars]
python_files = "./src"

[scripts]
# Scripts base reutilizables
lint = "pylint <axes::vars::python_files>"
test = "pytest <axes::vars::python_files>"

# Script compuesto que une los anteriores.
quality = [
    "echo '🚀 Ejecutando todas las comprobaciones de calidad...'",
    "<axes::scripts::lint>",
    "<axes::scripts::test>",
    "echo '✅ Todo en orden!'"
]
```

Ahora, un simple `axes mi-app quality` ejecuta un flujo de trabajo completo. Si mañana decides cambiar `pylint` por `ruff`, solo tienes que cambiarlo en un lugar, y `quality` seguirá funcionando.

### 4.4. Ejecución y Sustitución: Scripts Dinámicos

A veces, necesitas el **resultado** de un comando para usarlo en otro. El token `<axes::run::...>` te permite hacer exactamente eso.

* **`<axes::run::comando_a_ejecutar>`:** `axes` ejecutará `comando_a_ejecutar`, capturará su salida estándar (stdout), la limpiará (eliminando espacios y saltos de línea), y la inyectará en el comando principal.

**Ejemplo: Etiquetado de Docker con el Hash de Git:**

```toml
[scripts]
# Un script privado (convención de empezar con '_') para obtener la versión.
_get_git_version = "git rev-parse --short HEAD"

# Construye la imagen Docker, usando la salida del script anterior como tag.
build_and_tag = "docker build -t mi-app:<axes::run::scripts::_get_git_version> ."
```

Al ejecutar `axes . build_and_tag`:

1. `axes` ve el token `<axes::run::scripts::_get_git_version>`.
2. Ejecuta el script `_get_git_version`, que ejecuta `git rev-parse --short HEAD`.
3. La salida de git (ej. `a1b2c3d`) es capturada.
4. El comando final se construye como `docker build -t mi-app:a1b2c3d .` y se ejecuta.

Este mecanismo te permite crear flujos de trabajo increíblemente dinámicos sin salir de la comodidad y legibilidad de tu `axes.toml`.

## 5. Scripts como Funciones: El Sistema de Parámetros (`<axes::params::...>`)

`axes` no solo ejecuta scripts; te permite definir verdaderas "funciones" de línea de comandos que aceptan argumentos de forma estructurada. Esto elimina la necesidad de escribir complejos scripts de `bash` para parsear flags y parámetros.

Toda la lógica de parámetros se controla a través del namespace `<axes::params::...>`.

> **Regla de Oro:** Si pasas argumentos a un script desde la línea de comandos (`axes . mi-script arg1 --flag`), el `axes.toml` de ese script **debe** usar al menos un token `<axes::params::...>` para consumirlos. Si no, `axes` devolverá un error para prevenir un comportamiento inesperado.

### 5.1. Parámetros Posicionales

Son los argumentos que se pasan sin un flag. Se acceden por su índice (empezando en 0).

* **Sintaxis:** `<axes::params::0>`, `<axes::params::1>`, etc.
* **Comportamiento:** Se reemplaza por el argumento posicional en ese índice. Si el argumento no existe, se reemplaza por una cadena vacía.

**Ejemplo: Un script de `git commit` simplificado.**

```toml
[scripts]
# Acepta un mensaje de commit como primer argumento posicional.
commit = "git commit -m \"<axes::params::0>\""
```

**Ejecución:**

```sh
# El '0' se refiere a "Fix: ..."
axes . commit "Fix: Corrige el bug de autenticación"

# Comando ejecutado:
# git commit -m "Fix: Corrige el bug de autenticación"
```

### 5.2. Parámetros Nombrados (Flags)

Puedes hacer que tus scripts reaccionen a flags (`--nombre`) pasados desde la CLI. Tienes dos formas de usarlos:

#### **A. Mapeo de Presencia (El más común)**

Esto te permite insertar un valor estático en tu comando solo si un flag está presente.

* **Sintaxis:** `<axes::params::nombre_flag='valor_a_insertar'>`
* **Comportamiento:** Si se pasa `--nombre_flag` en la CLI, el token se reemplaza por `'valor_a_insertar'`. Si no, se reemplaza por una cadena vacía.

**Ejemplo: Un script de `build` con un flag opcional `--release`.**

```toml
[scripts]
build = "cargo build <axes::params::rel='--release'>"
```

**Ejecución:**

```sh
# Ejecuta el build en modo debug
axes . build
# Comando ejecutado: `cargo build ` (el token se expande a nada)

# Ejecuta el build en modo release
axes . build --rel
# Comando ejecutado: `cargo build --release`
```

#### **B. Paso a Través Directo (Passthrough)**

Esto busca un flag en la CLI y lo reinyecta, junto con su valor si lo tiene.

* **Sintaxis:** `<axes::params::nombre_flag>`
* **Comportamiento:**
  * Si se ejecuta con `--nombre_flag valor`, el token se reemplaza por `"--nombre_flag" "valor"`.
  * Si se ejecuta con `--nombre_flag`, el token se reemplaza por `"--nombre_flag"`.
  * Si no se pasa el flag, se reemplaza por una cadena vacía.

**Ejemplo: Un script de `test` que puede pasar un flag `--marker` a `pytest`.**

```toml
[scripts]
test = "pytest <axes::params::marker>"
```

**Ejecución:**

```sh
# Ejecuta todos los tests
axes . test
# Comando ejecutado: `pytest `

# Ejecuta solo los tests marcados como 'slow'
axes . test --marker slow
# Comando ejecutado: `pytest --marker slow`
```

### 5.3. El Recolector Genérico: `<axes::params>`

Este es el token "recolector". Es útil cuando quieres pasar un número variable de argumentos o flags a un comando subyacente sin tener que definirlos todos explícitamente.

* **Sintaxis:** `<axes::params>`
* **Comportamiento:** Se reemplaza por **todos los argumentos** (posicionales y nombrados) que **no fueron consumidos** por un token explícito (`::0`, `::flag=...`, etc.).

**Ejemplo: Un wrapper genérico para `npm install`.**

```toml
[scripts]
# Pasa todos los argumentos directamente a `npm install`.
add = "npm install <axes::params>"
```

**Ejecución:**

```sh
# Instala una dependencia de desarrollo
axes . add --save-dev react
# Comando ejecutado: `npm install --save-dev react`

# Instala una dependencia específica
axes . add typescript@latest
# Comando ejecutado: `npm install typescript@latest`
```

Combinando estos patrones, puedes construir interfaces de línea de comandos increíblemente ricas y robustas para tus proyectos, todo dentro de la simplicidad de `axes.toml`.

## 6. Opciones de Entorno y Hooks

Además de los scripts, `axes` te permite definir configuraciones que afectan a cómo se ejecutan todos los comandos y cómo se comportan las sesiones interactivas.

### 6.1. Variables de Entorno `[env]`

Cualquier par clave-valor que definas en la sección `[env]` se inyectará como una variable de entorno en la sub-shell donde se ejecutan tus scripts. Esto es ideal para configurar credenciales, URLs de bases de datos, o flags de comportamiento para tus herramientas.

**Comportamiento:**

* Las variables de `[env]` se heredan y se fusionan de padres a hijos.
* Las definiciones en el hijo siempre sobrescriben las del padre si tienen la misma clave.

**Sintaxis:**

```toml
[env]
NOMBRE_VARIABLE = "valor"
OTRA_VARIABLE = "otro valor"
```

#### **Ejemplo Práctico: Configuración de una Aplicación Web**

```toml
# en el `axes.toml` del proyecto raíz `mi-app`
[env]
# Variable común para todos los entornos
DATABASE_URL = "postgres://user:pass@localhost/db"
# Por defecto, el entorno es de desarrollo
APP_ENV = "development"

# en el `axes.toml` del hijo `mi-app/api-tests`
[env]
# Sobrescribe la variable del padre solo para este contexto de pruebas.
APP_ENV = "testing"
```

Cuando ejecutes cualquier script desde `mi-app/api-tests`, la variable `APP_ENV` tendrá el valor `"testing"`. Para cualquier otro proyecto hijo, será `"development"`.

### 6.2. Opciones y Hooks de Sesión `[options]`

La sección `[options]` te permite personalizar el comportamiento del comando `start`.

#### **Hooks de Sesión: `at_start` y `at_exit`**

Estos son scripts que se ejecutan automáticamente al entrar y salir de una sesión interactiva.

* `at_start`: Un comando que se ejecuta **antes** de que obtengas el control de la terminal en una sesión. Perfecto para activar entornos virtuales, establecer variables de sesión complejas o iniciar servicios en segundo plano.
* `at_exit`: Un comando que se ejecuta **después** de que sales de la sesión con `exit`. Ideal para tareas de limpieza como detener contenedores de Docker o eliminar archivos temporales.

#### **Ejemplo: Un Entorno de Python con Docker**

```toml
[options]
# Al iniciar la sesión, activa el venv y levanta la DB.
at_start = """
source .venv/bin/activate &&
docker-compose up -d
"""

# Al salir, detiene y elimina los contenedores.
at_exit = "docker-compose down"
```

#### **Personalización de la Shell: `shell`**

Por defecto, `axes` intenta usar la shell predeterminada de tu sistema (`bash`, `cmd`). Puedes forzar el uso de una shell específica para un proyecto.

```toml
[options]
# Usa zsh para este proyecto, quizás porque `at_start` usa una función de zsh.
shell = "zsh"
```

#### **Configuración del Comando `open`: `[options.open_with]`**

Esta sub-sección te permite definir los atajos para el comando `axes <ctx> open`.

**Sintaxis:**

```toml
[options.open_with]
# La clave es el nombre del atajo.
# El valor es el comando a ejecutar.
# Usa el token <axes::path> para referirte al directorio raíz del proyecto.
nombre_atajo = "comando --que-usa \"<axes::path>\""
default = "nombre_atajo_por_defecto"
```

**Ejemplo Completo:**

```toml
[options.open_with]
# Atajo `edit` para abrir en VS Code Insiders.
# Usa una variable para que sea fácil de cambiar globalmente.
edit = "<axes::vars::editor_cmd> \"<axes::path>\""

# Atajo `files` para el explorador de archivos.
files = "explorer \"<axes::path>\"" # `explorer` en Windows, `open` en macOS, `xdg-open` en Linux

# Atajo `terminal` para abrir una nueva terminal en esa ruta.
terminal = "wt -d \"<axes::path>\"" # `wt` es Windows Terminal

# Define `edit` como la acción por defecto al ejecutar `axes . open`.
default = "edit"

[vars]
editor_cmd = "code-insiders"
```

Con esta configuración en tu proyecto `global`, todos tus proyectos heredarán estos atajos de `open` súper útiles.

---

## Conclusión

Ahora tienes el conocimiento completo para escribir archivos `axes.toml` potentes y bien estructurados. Has aprendido a:

* Definir **variables** para reutilizar valores.
* Crear **scripts** simples, secuenciales, y multiplataforma.
* Usar el **motor de expansión `<axes::...>`** para componer scripts y usar metadatos.
* Crear **scripts parametrizables** que actúan como funciones de CLI.
* Configurar el **entorno de ejecución** y los **hooks de sesión**.

El siguiente paso es explorar la referencia de todos los comandos de la CLI para ver cómo interactúan con tus proyectos.

➡️ **Siguiente Lectura Recomendada: [Referencia Completa de Comandos (`COMMANDS.md`)](./COMMANDS.md)**
