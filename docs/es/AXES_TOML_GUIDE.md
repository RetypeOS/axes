<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../AXES_TOML_GUIDE.md">English</a> •
  <a href="./AXES_TOML_GUIDE.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../AXES_TOML_GUIDE.md), que es la fuente canónica de la documentación.

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
build = "npm run build -- --output <vars::dist_dir>"
serve = "npm run serve"

# --- Opciones y Hooks ---
[options]
# Se ejecuta al iniciar una sesión con `axes . start`
at_start = "nvm use 18"
# Se ejecuta al salir de la sesión
at_exit = "echo 'Limpiando sesión...'"

# Configuración para el comando `axes . open`
[options.open_with]
editor = "code \"<path>\""
default = "editor"
```

---

## 1. Metadatos (Opcional)

Estas claves son puramente informativas y ayudan a documentar tu proyecto.

* `version`: La versión de tu proyecto (ej. `"1.0.0"`). Es accesible en los scripts a través del token `<version>`.
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
Para usar una variable, utiliza la sintaxis `<vars::nombre_variable>`. `axes` reemplazará el token con el valor de la variable antes de ejecutar el comando.

```toml
[scripts]
# Usa las variables definidas arriba.
build = "c++ <vars::compiler_flags> -o <vars::output_dir>/app main.cpp"
```

Las variables también pueden componerse entre sí y usar otros tokens de `axes`:

```toml
[vars]
# El directorio de artefactos depende del nombre del proyecto.
artifact_dir = "artifacts/<name>"
# El nombre del archivo final se compone de otra variable.
final_zip = "<vars::artifact_dir>/<name>.zip"
```

## 3. Scripts y Flujos de Trabajo `[scripts]`

Esta es la sección principal de `axes`. Un "script" es un punto de entrada con nombre para una tarea que quieres realizar. Cada clave en la tabla `[scripts]` define un comando que puedes ejecutar con `axes <ctx> <nombre_script>`.

`axes` ofrece una sintaxis increíblemente flexible, permitiéndote definir desde un simple alias hasta un flujo de trabajo multiplataforma complejo.

### 3.1. Sintaxis de Comandos

Puedes definir un comando de varias formas, desde la más simple a la más completa.

#### **A. Comando Simple (String)**

La forma más básica. `axes` lo tratará como el comando por defecto para tu sistema operativo actual.

```toml
[scripts]
# Comprueba el código en busca de errores sin compilar.
check = "cargo check"

# Inicia un servidor de desarrollo simple.
serve = "python -m http.server 8000"
```

#### **B. Secuencia de Comandos (Array de Strings)**

Para flujos de trabajo que requieren múltiples pasos, define el script como una lista de cadenas. `axes` ejecutará cada comando en orden y se detendrá si alguno de ellos falla (a menos que uses modificadores).

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

#### **C. Estructura Extendida (Tabla)**

Para añadir una descripción o definir un comportamiento multiplataforma, usa una tabla TOML.

* **Con descripción:**

    ```toml
    [scripts]
    lint = { desc = "Ejecuta el linter para encontrar problemas de estilo.", run = "eslint ." }
    test = { desc = "Ejecuta la suite de tests completa.", run = ["npm run test:unit", "npm run test:e2e"] }
    ```

    La `desc` se mostrará en comandos como `axes . info`. La clave `run` puede ser un string o un array, como en los casos anteriores.

* **Multiplataforma:**
    Define un único script que se comporta de forma diferente según el sistema operativo. `axes` seleccionará automáticamente el comando correcto.

    ```toml
    [scripts.browse]
    desc = "Abre la documentación local en el navegador por defecto."
    windows = "start http://localhost:8080"
    macos = "open http://localhost:8080"
    linux = "xdg-open http://localhost:8080"
    # `default` se usa si el SO actual no coincide con ninguno de los anteriores.
    default = "echo 'Visita http://localhost:8080 en tu navegador.'"
    ```

### 3.2. Modificadores de Ejecución (`-` y `>`)

Puedes controlar cómo se ejecuta cada línea en una secuencia usando prefijos especiales.

> **Regla Clave:** Los modificadores solo tienen efecto en la línea donde están escritos. **No se "heredan"** cuando un script es compuesto por otro. El control de la ejecución siempre pertenece al script "llamador".

#### **Ignorar Errores con `-`**

Normalmente, si un comando en una secuencia falla, toda la secuencia se detiene. A veces, quieres que un comando se ejecute pero no te importa si falla. Prefija ese comando con `-` para que `axes` ignore su código de salida y continúe con el siguiente paso.

```toml
[scripts]
# Intenta limpiar la caché, pero no falles si el directorio no existe.
build = [
    "-rm -rf .cache",
    "npm run build"
]
```

Aquí, si `rm` falla, `axes` continuará y ejecutará `npm run build`.

#### **Ejecución Paralela con `>`**

Si prefijas un comando con `>` en una secuencia, `axes` lo lanza y continúa inmediatamente con el siguiente, sin esperar a que termine. Esto es ideal para iniciar procesos de larga duración como servidores de desarrollo o watchers.

```toml
[scripts]
# Inicia los servidores de backend y frontend simultáneamente.
dev = [
    "> axes api dev",
    "> axes frontend dev"
]
```

Al ejecutar `axes . dev`, `axes` lanzará el script `dev` de `api` y, un instante después, el script `dev` de `frontend`. `axes` esperará a que todos los procesos lanzados en paralelo terminen antes de dar por finalizada la tarea principal.

### 3.3. Composición de Scripts: El Corazón de la Reutilización

Una de las características más potentes de `axes` es la capacidad de construir scripts complejos a partir de piezas más pequeñas y reutilizables.

* **Sintaxis:** `<scripts::nombre_otro_script>`

Cuando `axes` expande tus scripts, reemplazará este token con el **contenido de texto puro** del script referenciado.

**Ejemplo de un Flujo de Calidad de Código:**

```toml
# en `mi-app/.axes/axes.toml` (el padre)
[scripts]
# Scripts base reutilizables
lint = { desc = "Ejecuta el linter.", run = "ruff check ." }
test = { desc = "Ejecuta los tests.", run = "pytest" }

# Script compuesto que une los anteriores.
# El control de ejecución (secuencial) pertenece a `quality`.
quality = [
    "echo '🚀 Ejecutando todas las comprobaciones de calidad...'",
    "<scripts::lint>",
    "<scripts::test>",
    "echo '✅ Todo en orden!'"
]
```

Ahora, un simple `axes mi-app quality` ejecuta `ruff check .` y luego `pytest`. Si mañana decides que el `lint` debe ejecutarse en paralelo, modificarías `quality`:

```toml
# Modificando `quality` para que el `lint` no bloquee (ejemplo hipotético)
quality = [
    "> <scripts::lint>",
    "<scripts::test>"
]
```

El `>` se aplica al *resultado* de la expansión de `<scripts::lint>`. La definición original de `lint` no cambia y puede seguir siendo usada de forma secuencial en otros scripts.

## 4. El Motor de Expansión: Dando Superpoderes a tus Scripts

La característica que une todo en `axes` es su motor de expansión de tokens. Cualquier valor de cadena en tu `axes.toml` (en `scripts`, `vars`, `options`, etc.) puede contener tokens especiales con el formato `<...>` que `axes` procesará antes de ejecutar el comando.

Este sistema te permite crear flujos de trabajo dinámicos, componibles y conscientes del contexto. La expansión ocurre de forma perezosa y sus resultados se guardan en un caché binario (`.axes/config.cache.bin`), haciendo que las ejecuciones subsecuentes sean extremadamente rápidas.

### 4.1. Tokens Estáticos (Metadatos y Variables)

Estos tokens se resuelven a valores de texto simples y se inyectan antes de cualquier otra cosa.

#### **Tokens de Metadatos del Proyecto**

Estos tokens te dan acceso a la información intrínseca del proyecto.

| Token             | Valor de Expansión                                                  | Ejemplo de Uso                                              |
| :---------------- | :------------------------------------------------------------------ | :---------------------------------------------------------- |
| `<name>`    | El nombre cualificado completo del proyecto.                        | `echo 'Construyendo <name>...'` -> `Construyendo mi-app/api...`             |
| `<path>`    | La ruta física (absoluta y limpia) al directorio raíz del proyecto. | `docker build -t app . -f "<path>/Dockerfile"`                             |
| `<uuid>`    | El identificador único universal del proyecto.                      | `aws s3 cp ... s3://bucket/<uuid>/`                                        |
| `<version>` | La versión definida en el `axes.toml` del proyecto.                 | `echo 'Desplegando versión <version>'` -> `Desplegando versión 1.2.0-beta`         |

#### **Tokens de Variables**

Estos tokens te permiten inyectar los valores que has definido en las secciones `[vars]` y `[env]`.

* **`<vars::nombre_variable>`:** Se expande al valor de la variable definida en la sección `[vars]`. `axes` buscará la variable en el `axes.toml` del proyecto actual y luego subirá por el árbol de herencia hasta que la encuentre.
* **`<env::NOMBRE_VARIABLE>`:** Se expande al valor de la variable definida en `[env]`. Funciona igual que las `vars` a nivel de herencia.

**Ejemplo Combinado:**

```toml
# en el `axes.toml` del padre `mi-app`
[vars]
docker_registry = "registry.example.com/mi-org"

# en el `axes.toml` del hijo `mi-app/api`
[scripts]
# Construye y etiqueta una imagen Docker con el nombre del proyecto y el registro del padre.
docker_build = "docker build -t <vars::docker_registry>/<name>:<version> ."
```

### 4.2. Tokens de Composición (Scripts y Variables Anidadas)

Esta es una de las características más potentes. Puedes construir flujos de trabajo complejos a partir de piezas más pequeñas.

* **`<scripts::nombre_otro_script>`:** `axes` reemplazará este token con el **contenido de texto puro** del script `nombre_otro_script` (ya resuelto para tu plataforma). Los prefijos de ejecución (`-`, `>`) del script anidado **no se heredan**; el control de la ejecución siempre pertenece al script que hace la llamada.

**Ejemplo de un Flujo de Calidad de Código:**

```toml
# en `mi-app/.axes/axes.toml` (el padre)
[vars]
python_files = "./src"

[scripts]
lint = "pylint <vars::python_files>"
test = "pytest <vars::python_files>"

# Script compuesto que une los anteriores.
quality = [
    "echo '🚀 Ejecutando todas las comprobaciones de calidad...'",
    "<scripts::lint>",
    "<scripts::test>",
    "echo '✅ Todo en orden!'"
]
```

Un simple `axes mi-app quality` ejecuta un flujo de trabajo completo. Si decides que el linter es opcional, solo modificas `quality`: `"-<scripts::lint>"`.

### 4.3. Ejecución y Sustitución: `<run::...>`

A veces, necesitas el **resultado** de un comando para usarlo en otro. El token `<run::...>` te permite hacer exactamente eso.

* **`<run::comando_a_ejecutar>`:** `axes` ejecutará `comando_a_ejecutar`, capturará su salida estándar (stdout), la limpiará (eliminando espacios y saltos de línea al final), y la inyectará en el comando principal.

**Ejemplo: Etiquetado de Docker con el Hash de Git:**

```toml
[scripts]
# Un script privado para obtener la versión.
_get_git_version = "git rev-parse --short HEAD"

# Construye la imagen Docker, usando la salida del script anterior como tag.
# Nota cómo componemos un <scripts::...> dentro de un <run::...>.
build_and_tag = "docker build -t mi-app:<run::<scripts::_get_git_version>> ."
```

Al ejecutar `axes . build_and_tag`:

1. `axes` ve el token `<run::...>` y primero expande su contenido.
2. `<scripts::_get_git_version>` se expande a `"git rev-parse --short HEAD"`.
3. `axes` ejecuta `git rev-parse --short HEAD`.
4. La salida de git (ej. `a1b2c3d`) es capturada.
5. El comando final se construye como `docker build -t mi-app:a1b2c3d .` y se ejecuta.

## 5. Scripts como Funciones: El Sistema de Parámetros (`<params::...`)

`axes` no solo ejecuta scripts; te permite definir verdaderas "funciones" de línea de comandos que aceptan argumentos de forma estructurada. Esto elimina la necesidad de escribir complejos scripts de `bash` para parsear flags y parámetros.

Toda la lógica de parámetros se controla a través del namespace `<params::...>` y sigue un **paradigma declarativo**: defines los parámetros que tu script espera, y `axes` valida la entrada del usuario **antes** de ejecutar nada.

> **Regla de Oro:** Si pasas argumentos a un script desde la línea de comandos (`axes . mi-script arg1 --flag`), el `axes.toml` de ese script **debe** usar tokens `<params::...>` para consumirlos. Si al final sobran argumentos que no fueron consumidos por ningún token (y no hay un `<params>` genérico), `axes` devolverá un error.

### 5.1. Parámetros Posicionales

Son los argumentos que se pasan sin un flag. Se acceden por su índice (empezando en 0).

* **Sintaxis Básica:** `<params::0>`, `<params::1>`, etc.
* **Comportamiento:** Se reemplaza por el argumento posicional en ese índice. Si el argumento no existe y no es requerido ni tiene un `default`, se reemplaza por una cadena vacía.

#### **Modificadores para Posicionales `(...)`**

* `required`: La ejecución falla si el argumento no se proporciona.
* `default='valor'`: Proporciona un valor por defecto si el argumento no se pasa en la CLI.
* `map='--nuevo-flag'`: Transforma el argumento posicional en un flag con valor. Si se proporciona `mi-valor`, el token se expande a `"--nuevo-flag mi-valor"`.

**Ejemplo: Un script de `git commit` simplificado.**

```toml
[scripts]
# Acepta un mensaje de commit como primer argumento posicional requerido.
commit = "git commit -m \"<params::0(required)>\""
```

**Ejecución:**

```sh
# El '0' se refiere a "Fix: ..."
axes . commit "Fix: Corrige el bug de autenticación"

# Comando ejecutado:
# git commit -m "Fix: Corrige el bug de autenticación"

# Falla si no se proporciona:
axes . commit
# -> Error: Positional argument at index 0 is required but was not provided.
```

### 5.2. Parámetros Nombrados (Flags)

Puedes hacer que tus scripts reaccionen a flags (`--nombre`) pasados desde la CLI.

* **Sintaxis Básica:** `<params::nombre-flag>`
* **Comportamiento por Defecto (Pass-through):** El token busca el flag en la CLI y lo reinyecta tal cual, junto con su valor si lo tiene. Si no se encuentra, se expande a una cadena vacía.

#### **Modificadores para Flags `(...)`**

* `required`: La ejecución falla si el flag (o su alias) no está presente.
* `default='valor'`: Si el flag **no se proporciona en absoluto**, se usará este `default`. También se aplica si el flag se proporciona **sin un valor** (ej. `comando --mi-flag`).
* `alias='-a'`: Permite que el flag sea reconocido por un alias corto. `axes` lanzará un error si el usuario proporciona tanto el nombre completo como el alias.
* `map='--nuevo-nombre'`: Reemplaza el nombre del flag en la salida. Muy útil para abstraer las herramientas subyacentes.
* `map=''`: Un caso especial. Indica que solo quieres inyectar el **valor** del flag, no el nombre del flag en sí. Ideal para inyectar valores en posiciones donde no se espera un flag.

**Ejemplo: Un script de `test` que puede pasar un flag `--marker` a `pytest`.**

```toml
[scripts]
# Usa el pass-through por defecto con un alias.
test = "pytest <params::marker(alias='-m')>"
```

**Ejecución:**

```sh
# Ejecuta todos los tests
axes . test
# Comando ejecutado: `pytest`

# Ejecuta solo los tests marcados como 'slow'
axes . test --marker slow
# Comando ejecutado: `pytest --marker slow`

# Usa el alias
axes . test -m smoke
# Comando ejecutado: `pytest -m smoke`
```

**Ejemplo: Un script de `deploy` con `map` y `default`.**

```toml
# axes.toml
[scripts]
# El script interno espera --environment, pero exponemos --env al usuario.
# Por defecto, se despliega en 'staging'.
deploy = "terraform apply -var 'env=<params::env(map='', default='staging')>'"
```

**Ejecución:**

```sh
# Usa el default
axes . deploy
# Comando ejecutado: terraform apply -var 'env=staging'

# Especifica un entorno
axes . deploy --env production
# Comando ejecutado: terraform apply -var 'env=production'
```

### 5.3. El Recolector Genérico: `<params>`

Este es el token "recolector". Es útil cuando quieres pasar un número variable de argumentos o flags a un comando subyacente sin tener que definirlos todos explícitamente.

* **Sintaxis:** `<params>`
* **Comportamiento:** Se reemplaza por **todos los argumentos** (posicionales y nombrados) que **no fueron consumidos** por un token explícito (`::0`, `::flag`, etc.), manteniendo su orden original.

**Ejemplo: Un `wrapper` genérico para `npm install` que también define `--save-dev`.**

```toml
[scripts]
# `add` pasa todos los argumentos restantes a `npm install`.
# `add_dev` primero define `--save-dev`, y luego pasa el resto.
add = "npm install <params::save-dev(alias='-D')> <params>"
```

**Ejecución:**

```sh
# Instala una dependencia normal
axes . add react
# Comando ejecutado: `npm install react`

# Instala una dependencia de desarrollo
axes . add -D typescript
# `-D` es consumido por <...::save-dev> y se expande a `--save-dev`.
# `typescript` es consumido por <params>.
# Comando ejecutado: `npm install --save-dev typescript`

# Instala múltiples dependencias con flags adicionales
axes . add react react-dom --force
# Comando ejecutado: `npm install react react-dom --force`
```

Combinando estos patrones, puedes construir interfaces de línea de comandos increíblemente ricas y robustas para tus proyectos, todo dentro de la simplicidad de `axes.toml`.

## 6. Opciones de Entorno y Hooks

Además de los scripts, `axes` te permite definir configuraciones que afectan a cómo se ejecutan todos los comandos y cómo se comportan las sesiones interactivas.

### 6.1. Variables de Entorno `[env]`

Cualquier par clave-valor que definas en la sección `[env]` se inyectará como una variable de entorno en el subproceso donde se ejecutan tus scripts. Esto es ideal para configurar credenciales, URLs de bases de datos, o flags de comportamiento para tus herramientas. Las variables de `[env]` se heredan y se fusionan de padres a hijos.

```toml
# en el `axes.toml` del proyecto raíz `mi-app`
[env]
DATABASE_URL = "postgres://user:pass@localhost/db"
APP_ENV = "development"

# en el `axes.toml` del hijo `mi-app/api-tests`
[env]
# Sobrescribe la variable del padre solo para este contexto de pruebas.
APP_ENV = "testing"
```

### 6.2. Opciones y Hooks de Sesión `[options]`

La sección `[options]` te permite personalizar el comportamiento del comando `start` y `open`.

#### **Hooks de Sesión: `at_start` y `at_exit`**

Estos son scripts que se ejecutan automáticamente al entrar y salir de una sesión interactiva (`axes <ctx> start`).

* **`at_start`**: Un comando (o secuencia) que se ejecuta **antes** de que obtengas el control de la terminal en una sesión. Perfecto para activar entornos virtuales, establecer variables de sesión o iniciar servicios.
* **`at_exit`**: Un comando (o secuencia) que se ejecuta **después** de que sales de la sesión. Ideal para tareas de limpieza.

**Importante:** Desde la v0.1.8, `at_start` y `at_exit` son **scripts completos**. Pueden ser secuencias, tener descripciones y, lo más importante, **aceptar parámetros** pasados al comando `start`.

#### **Ejemplo: Un Entorno de Python con Docker y Parámetros**

```toml
[options]
at_start = { desc = "Activa el venv y levanta la DB.", run = [
    "source .venv/bin/activate",
    "docker-compose up -d <params::service(default='db')>"
]}
at_exit = { desc = "Detiene y elimina los contenedores.", run = "docker-compose down" }
```

**Ejecución:**

```sh
# Inicia la sesión y levanta el servicio 'db' por defecto
axes . start

# Inicia la sesión y especifica qué servicio levantar
axes . start --service web
```

#### **Personalización de la Shell: `shell`**

Por defecto, `axes` intenta usar la shell predeterminada de tu sistema. Puedes forzar el uso de una shell específica para un proyecto.

```toml
[options]
# Usa zsh para este proyecto.
shell = "zsh"
```

#### **Configuración del Comando `open`: `[options.open_with]`**

Esta sub-sección te permite definir los atajos para el comando `axes <ctx> open`. Al igual que los hooks de sesión, cada atajo es un **script completo** y puede aceptar parámetros.

**Ejemplo Completo:**

```toml
[options.open_with]
# Atajo `edit` para abrir en VS Code.
edit = { desc = "Abre el proyecto en VS Code.", run = "<vars::editor_cmd> \"<path>\"" }

# Atajo `files` para el explorador de archivos.
files = { desc = "Abre el directorio en el explorador de archivos.", run = "explorer \"<path>\"" } # `explorer` en Windows, `open` en macOS, `xdg-open` en Linux

# Atajo `terminal` que acepta un parámetro para abrir una subcarpeta.
terminal = "wt -d \"<path>/<params::0(default='.')>\"" # `wt` es Windows Terminal

# Define `edit` como la acción por defecto al ejecutar `axes . open`.
default = "edit"

[vars]
editor_cmd = "code"
```

**Ejecución:**

```sh
# Abre el proyecto con el editor por defecto ('edit')
axes . open

# Abre el explorador de archivos
axes . open files

# Abre una nueva terminal en el subdirectorio 'src'
axes . open terminal src
```

Con esta configuración en tu proyecto `global`, todos tus proyectos heredarán estos atajos de `open` muy útiles.

---

## Conclusión

Ahora tienes el conocimiento completo para escribir archivos `axes.toml` potentes y bien estructurados. Has aprendido a:

* Definir **variables** para reutilizar valores.
* Crear **scripts** simples, secuenciales, y multiplataforma.
* Usar el **motor de expansión `<...>`** para componer scripts y usar metadatos.
* Crear **scripts parametrizables** que actúan como funciones de CLI.
* Configurar el **entorno de ejecución** y los **hooks de sesión**.

El siguiente paso es explorar la referencia de todos los comandos de la CLI para ver cómo interactúan con tus proyectos.

➡️ **Siguiente Lectura Recomendada: [Referencia Completa de Comandos (`COMMANDS.md`)](./COMMANDS.md)**
