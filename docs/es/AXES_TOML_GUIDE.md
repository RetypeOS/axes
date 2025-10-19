<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../AXES_TOML_GUIDE.md">English</a> •
  <a href="./AXES_TOML_GUIDE.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../AXES_TOML_GUIDE.md), que es la fuente canónica de la documentación.

# Dominando `axes.toml`: La Guía Definitiva

El archivo `axes.toml` es el cerebro de cada uno de tus proyectos. Aquí es donde transformas secuencias de comandos caóticas en flujos de trabajo limpios, reutilizables y potentes. Esta guía es la referencia completa para cada sección y característica que puedes utilizar.

## El Principio Fundamental: Herencia

Antes de sumergirnos en los detalles, recuerda el concepto más importante: la **herencia**.

Cada proyecto `axes` hereda la configuración completa de su padre. Cuando `axes` necesita un valor (como un script o una variable), sigue una ruta de búsqueda clara:

1. Busca en el `axes.toml` del **proyecto actual**.
2. Si no lo encuentra, busca en el `axes.toml` del **padre**.
3. ...y así sucesivamente, subiendo hasta el proyecto **`global`**.

El **primer valor encontrado gana**. Esto significa que la configuración de un hijo siempre tiene precedencia y puede **anular** las definiciones de sus padres. Las variables de entorno (`[env]`) son la única excepción: se **fusionan**, con los valores del hijo sobrescribiendo los del padre para la misma clave.

### Anatomía de un `axes.toml`

Aquí tienes un ejemplo de un `axes.toml` con todas las secciones principales. Las exploraremos una por una.

```toml
# ==============================================================================
# axes.toml: Guía de Referencia Completa
# Este archivo sirve como un ejemplo exhaustivo de todas las características disponibles en `axes`.
# ==============================================================================

# --- 1. Metadatos (Opcional) ---
# Proporciona información sobre el proyecto, visible con `axes info`.
version = "2.0.0"
description = "API de Backend para el proyecto WebApp. Proporciona endpoints de datos."

# --- 2. Variables de Entorno ([env]) ---
# Estas variables se inyectan como variables de entorno del sistema en CADA comando ejecutado por `axes` en este contexto.
[env]
# Ideal para secretos (si se define en un proyecto ancestro exterior) o constantes de entorno.
DATABASE_URL = "postgresql://user:pass@localhost:5432/webapp_db"
LOG_LEVEL = "info"

# --- 3. Variables de `axes` ([vars]) ---
# Variables internas para reutilización dentro de scripts usando la sintaxis `<vars::...>`.
# Promueven la filosofía DRY (Don't Repeat Yourself).
[vars]
image_name = "webapp/api"
# Las variables pueden ser dinámicas, ejecutando un comando en tiempo real.
git_hash = "<run('git rev-parse --short HEAD')>"

# --- 4. Scripts ([scripts]) ---
# El núcleo de `axes`. Define los flujos de trabajo del proyecto.
[scripts]

# Forma simple: un solo comando como cadena de texto.
run = "poetry run uvicorn app.main:app --reload"

# Forma de secuencia: una lista de comandos ejecutados secuencialmente.
# Usa '#' para imprimir mensajes de estado sin invocar un shell.
test = [
    "# Ejecutando pruebas de API...",
    "poetry run pytest"
]

# Forma extendida: un diccionario con una descripción (`desc`) y el comando (`run`).
# Esto mejora la salida de `axes info` y `axes run` (sin argumentos).
[scripts.seed_db]
desc = "Rellena la base de datos con datos de prueba."
run = [
  "# Aplicando seeds a la base de datos...",
  # `run` puede contener líneas multiplataforma. `axes` elegirá la correcta.
  # Si la específica del SO no existe, recurre a `default`.
  { windows = "psql.exe -U user -d webapp_db -f ./seed.sql", default = "psql -U user -d webapp_db -f ./seed.sql" }
]

# Script con un parámetro nombrado (`tag`) que tiene un valor por defecto.
[scripts.build]
desc = "Construye la imagen Docker local."
run = "docker build . -t <vars::image_name>:<params::tag(default='latest')>"

# Script que delega el parseo de argumentos al shell usando el prefijo '$'.
# Permite pasar flags y argumentos directamente al comando subyacente.
# Ejemplo de uso: `axes format --check .` se convierte en `poetry run ruff format .`
[scripts.format]
desc = "Formatea el código usando Ruff."
run = "$ poetry run ruff format ."

# Un script complejo que demuestra composición y modificadores de comandos.
[scripts.deploy]
desc = "Construye y empuja la imagen Docker de la API."
run = [
  "# Paso 1: Construir la imagen (ejecución silenciosa, el comando no se imprime).",
  "@ <scripts::build>", # <-- Composición: llama a otro script de `axes`.

  "# Paso 2: Etiquetar la imagen con el hash de commit (ignora errores si la etiqueta ya existe).",
  "- docker tag <vars::image_name>:latest <vars::image_name>:<vars::git_hash>",

  "# Paso 3: Empujar ambas etiquetas en paralelo para máxima velocidad.",
  "> docker push <vars::image_name>:latest", # <-- El prefijo `>` inicia un lote paralelo.
  "> docker push <vars::image_name>:<vars::git_hash>"
]


# --- 5. Opciones y Hooks de Sesión ([options]) ---
[options]

# `at_start`: Se ejecuta una vez al iniciar una sesión con `axes start`.
# Ideal para activar entornos virtuales, iniciar servicios, etc.
at_start = "poetry install --no-root"

# `at_exit`: Se ejecuta al salir de la sesión (con `exit`).
# Ideal para detener servicios, limpiar archivos temporales, etc.
at_exit = "# Saliendo de la sesión de API..."

# Configuración para el comando `axes open`.
[options.open_with]
# Define "atajos" para abrir el proyecto en diferentes aplicaciones.
# `<path>` es un token especial que resuelve a la ruta raíz del proyecto.
editor = "code \"<path>\""
terminal = { windows = "wt -d \"<path>\"", default = "gnome-terminal --working-directory=\"<path>\""}

# `default` especifica qué atajo usar si `axes open` se ejecuta sin argumentos.
default = "editor"
```

---

## 1. Metadatos (Opcional)

Estas claves son puramente informativas y ayudan a documentar tu proyecto.

* `version`: La versión de tu proyecto (ej., `"1.0.0"`). Es accesible en scripts a través del token `<version>`.
* `description`: Una descripción breve de lo que hace el proyecto. Se muestra en comandos como `info`.

```toml
version = "2.1.0-beta"
description = "El servicio principal de autenticación."
```

---

## 2. Variables de Interpolación `[vars]`

La sección `[vars]` es tu herramienta para el código DRY (Don't Repeat Yourself). Define valores una vez y reutilízalos en múltiples scripts a través del token `<vars::...>`

### Definición de Variables

Las variables deben resolverse a un valor de una sola línea.

**A. Forma Simple (String):**

```toml
[vars]
image_name = "my-app/api"
```

**B. Forma Extendida (Tabla):**
Usa una tabla para agregar una descripción o definir valores específicos de la plataforma. **Debes** usar la clave `value`.

```toml
[vars.binary_path]
desc = "Ruta al binario de la aplicación compilada."
value = { windows = "target\\release\\app.exe", default = "target/release/app" }
```

**Uso:**

```toml
[scripts]
run = "<vars::binary_path> --serve"
```

## 3. Scripts y Flujos de Trabajo `[scripts]`

Este es el núcleo de `axes`, donde defines las tareas de tu proyecto. Cada clave en la tabla `[scripts]` se convierte en un comando que puedes ejecutar.

### 3.1. Sintaxis del Comando

`axes` proporciona una sintaxis altamente flexible para definir scripts, desde simples líneas únicas hasta flujos de trabajo complejos y multiplataforma.

#### **A. Comando Simple (String)**

La forma más básica. Una sola cadena a ejecutar.

```toml
[scripts]
test = "cargo test -- --nocapture"
```

#### **B. Secuencia de Comandos (Array)**

Para flujos de trabajo de múltiples pasos. `axes` ejecuta cada comando en orden y se detiene si alguno falla.

```toml
[scripts]
deploy = [
    "# 1. Construyendo assets...",
    "npm run build",
    "# 2. Publicando al servidor...",
    "scp -r ./dist/* user@server:/var/www/my-app",
]
```

**o:**

```toml
[scripts]
deploy = [
    {default = "# 1. Building assets..."},
    {default = "npm run build"},
    {windows = "# 2. Publishing to server on Windows...", macos = "# 2. Publishing to server on Mac OS...", linux = "# 2. Publishing to server on Linux...", default = "# 2. Publishing to server on another OS..."},
    {default = "scp -r ./dist/* user@server:/var/www/my-app"},
]
```

Cada elemento en el array puede ser un `String` o un `Bloque de Plataforma` (ver más adelante).

> **Nota:** Los archivos `TOML` no permiten listas de diferentes tipos, por lo que si utilizas esta sintaxis, todo el script debe ser de tipo diccionario o cadena; no pueden combinarse.

#### **C. Estructura Extendida (Tabla)**

Para agregar una descripción o usar una sintaxis más avanzada, define el script como una tabla TOML.

* **Con clave `run`:**

    ```toml
    [scripts.lint]
    desc = "Ejecuta el linter para encontrar problemas de estilo."
    run = "eslint ." # `run` puede ser una String o un Array
    ```

* **Claves de Plataforma Directas (para scripts de una sola línea):**
    Esta es la sintaxis recomendada y ergonómica para comandos multiplataforma. La clave `run` no es necesaria.

    ```toml
    [scripts.browse]
    desc = "Abre la documentación local en el navegador predeterminado."
    windows = "start http://localhost:8080"
    macos = "open http://localhost:8080"
    linux = "xdg-open http://localhost:8080"
    # `default` es un fallback para otros sistemas.
    default = "echo 'Visita http://localhost:8080 en tu navegador.'"
    ```

* **Array de tablas para secuencias complejas (scripts multilínea):**
    Esta es la sintaxis más potente y explícita. Es ideal para scripts multilínea donde una o más líneas tienen lógica de plataforma. Usa `[[scripts.nombre.run]]` para cada paso en la secuencia.

    ```toml
    [scripts.browse]
    desc = "Abre la documentación local en el navegador predeterminado."

    [[run]]
    default = "# --- Iniciando servidor... por favor espera... ---"

    [[run]]
    windows = "start http://localhost:8080"
    macos = "open http://localhost:8080"
    linux = "xdg-open http://localhost:8080"
    default = "echo 'Visita http://localhost:8080 en tu navegador.'"

    [[run]]
    default = "# --- ¡Servidor abierto! ---"
    ```

> **Nota** Recomendamos encarecidamente que aprendas las formas óptimas de definir estas estructuras en TOML; hay otras formas más óptimas de definir estos datos.

El campo `desc` es altamente recomendado ya que mejora la salida de `axes info` y `axes run`.

### **3.2. Modificadores de Ejecución (Prefijos)**

Puedes controlar cómo se ejecuta cada línea en una secuencia usando prefijos especiales. Se pueden combinar (ej., `>- @ mi_comando`).

> **Regla Clave:** Los modificadores solo tienen efecto en la línea donde están escritos. **No son "heredados"** cuando un script es compuesto por otro. El control de ejecución siempre pertenece al script "llamador".

| Prefijo | Nombre                  | Descripción                                                                                                       |
| :----- | :---------------------- | :---------------------------------------------------------------------------------------------------------------- |
| `-`    | **Ignorar Errores**     | `axes` continuará con el siguiente comando en una secuencia incluso si este falla (sale con un código distinto de cero). |
| `>`    | **Ejecución Paralela**  | Agrupa este comando con todos los comandos `>` subsiguientes en un **lote**. `axes` ejecuta todos los comandos del lote concurrentemente y **espera a que todos terminen** antes de pasar al siguiente comando secuencial. |
| `@`    | **Modo Silencioso**     | `axes` no imprimirá el comando (`→ mi_comando`) en la consola antes de ejecutarlo. Útil para una salida limpia.      |
| `#`    | **Modo Echo**           | Toda la línea es tratada como una cadena para ser impresa en la consola, no como un comando a ejecutar.            |
| `\|`   | **Terminador**          | Indica explícitamente al analizador de prefijos que se detenga. Útil para comandos que comienzan con un carácter especial. |

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
# Inicia los servidores de backend y frontend simultáneamente.
dev = [
    "> axes api dev",
    "> axes frontend dev"
]
```

**Modo Silencioso & Echo (`@`, `#`):**

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
# El `-v` es una bandera para `mi_herramienta`, no un modificador para `axes`.
advanced = ">| -v --some-flag"
```

### 3.3. Composición de Scripts: El Corazón de la Reutilización

Una de las características más potentes de `axes` es su capacidad para construir scripts complejos a partir de piezas más pequeñas y reutilizables mediante la expansión de tokens **antes** de la ejecución.

* **Sintaxis:** `<scripts::otro_nombre_del_script>`

Cuando `axes` prepara tus scripts, los **compone estructuralmente**. Si llamas a un script de múltiples líneas, sus comandos se insertan directamente en la lista de comandos del padre.

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
    "> <scripts::test>", # `test` es secuencial en sí mismo, pero `quality` lo ejecuta en paralelo.
]
```

Al ejecutar `axes quality`, se ejecutará `ruff check .`, y una vez que termine, lanzará ambos comandos `pytest` en paralelo.

## 4. El Motor de Expansión: Potenciando tus Scripts

La característica que une todo es su motor de expansión de tokens. Cualquier valor de cadena en tu `axes.toml` puede contener tokens especiales en el formato `<...>` que `axes` procesará.

La expansión ocurre de manera perezosa, y sus resultados se guardan como un Árbol de Sintaxis Abstracta (AST) puro en una caché binaria (`.axes/config.cache.bin`), lo que hace que las ejecuciones posteriores sean extremadamente rápidas.

### 4.1. Tokens de Valor Estático

Estos tokens se resuelven a sus valores finales durante la fase de expansión (compilación JIT).

#### **Tokens de Metadatos del Proyecto**

| Token             | Valor de Expansión                                                  |
| :---------------- | :------------------------------------------------------------------ |
| `<name>`          | El nombre cualificado completo del proyecto (ej., `my-app/api`).    |
| `<path>`          | La ruta física absoluta al directorio raíz del proyecto.            |
| `<uuid>`          | El identificador único universal del proyecto.                      |
| `<version>`       | La versión definida en el `axes.toml` del proyecto.                 |

#### **Tokens de Variables**

* **`<vars::nombre_variable>`:** Se expande al valor de la variable definida en la sección `[vars]`.

**Ejemplo Combinado:**

```toml
# en el `axes.toml` del padre `my-app`
[vars]
docker_registry = "registry.example.com/my-org"

# en el `axes.toml` del hijo `my-app/api`
[scripts]
docker_build = "docker build -t <vars::docker_registry>/<name>:<version> ."
```

### 4.2. Token de Ejecución Dinámica: `<run::(...)>`

A veces, necesitas el **resultado** de un comando para usarlo en otro.

* **Sintaxis:** `<run('comando_a_ejecutar')>`
* **Comportamiento:** `axes` ejecuta `comando_a_ejecutar` **en tiempo de ejecución**, captura su salida estándar (stdout), la limpia (eliminando espacios en blanco finales) y la inyecta en el comando principal.

> **Importante:** La salida de los tokens `run` **nunca** se almacena en caché para asegurar que el dato sea siempre fresco.

**Ejemplo: Etiquetado de Docker con Hash de Git:**

```toml
[scripts]
tag_release = "docker tag my-app:latest my-app:<run('git rev-parse --short HEAD')>"
```

Al ejecutar `axes tag_release`:

1. `axes` se prepara para ejecutar el script `tag_release`.
2. Encuentra el token `<run::(...)>`.
3. Ejecuta `git rev-parse --short HEAD`.
4. La salida de git (ej., `a1b2c3d`) es capturada.
5. El comando final se ensambla como `docker build -t my-app:a1b2c3d .` y luego se ejecuta.

### 4.3. Tokens de Parámetros en Tiempo de Ejecución: `<params::...>`

Esta familia especial de tokens no se expande de antemano. Son marcadores de posición que son resueltos en el último momento por el `task_executor`, utilizando los argumentos que proporcionas en la línea de comandos.

(Esto se cubre en profundidad en la siguiente sección.)

## 5. Scripts como Funciones: El Sistema de Parámetros (`<params::...`)

`axes` no solo ejecuta scripts; te permite definir verdaderas "funciones" de línea de comandos que aceptan argumentos de forma estructurada. Esto elimina la necesidad de escribir scripts `bash` complejos para parsear banderas y parámetros.

Toda la lógica de parámetros se controla a través del espacio de nombres `<params::...>` y sigue un **paradigma declarativo**: defines los parámetros que tu script espera, y `axes` valida la entrada del usuario **antes** de ejecutar nada.

> **Regla de Oro:** Si pasas argumentos a un script desde la línea de comandos (`axes mi-script arg1 --flag`), el `axes.toml` de ese script **debe** usar tokens `<params::...>` para consumirlos. Si quedan argumentos sin consumir por ningún token (y no hay un token `<params>` genérico), `axes` devolverá un error.

### 5.1. Parámetros Posicionales

Estos son argumentos pasados sin una bandera. Se accede a ellos por su índice (comenzando en 0).

* **Sintaxis Básica:** `<params::0>`, `<params::1>`, etc.
* **Comportamiento:** Reemplazado por el argumento posicional en ese índice. Si el argumento no existe y no es requerido o no tiene un `default`, se reemplaza por una cadena vacía.

#### **Modificadores para Posicionales `(...)`**

* `required`: La ejecución falla si no se proporciona el argumento.
* `default='value'`: Proporciona un valor por defecto si el argumento no se pasa en la CLI.
* `map='--new-flag'`: Transforma el argumento posicional en una bandera con un valor. Si se proporciona `mi-valor`, el token se expande a `"--new-flag mi-valor"`.

**Ejemplo: Un script simplificado de `git commit`.**

```toml
[scripts]
# Acepta un mensaje de commit como el primer argumento posicional requerido.
commit = "git commit -m \"<params::0(required)>\""
```

**Ejecución:**

```sh
# El '0' se refiere a "Fix: ..."
axes commit "Fix: Arreglar bug de autenticación"

# Comando ejecutado:
# git commit -m "Fix: Arreglar bug de autenticación"

# Falla si no se proporciona:
axes commit
# -> Error: El argumento posicional en el índice 0 es requerido pero no fue proporcionado.
```

### 5.2. Parámetros Nombrados (Flags)

Puedes hacer que tus scripts reaccionen a las banderas (`--nombre`) pasadas desde la CLI.

* **Sintaxis Básica:** `<params::nombre-bandera>`
* **Comportamiento por Defecto (Pass-through):** El token busca la bandera en la CLI y la reinyecta tal cual, junto con su valor si lo tiene. Si no se encuentra, se expande a una cadena vacía.

#### **Modificadores para Banderas `(...)`**

* `required`: La ejecución falla si la bandera (o su alias) no está presente.
* `default='value'`: Si la bandera **no se proporciona en absoluto**, se usará este `default`. También se aplica si la bandera se proporciona **sin un valor** (ej., `comando --mi-bandera`).
* `alias='-a'`: Permite que la bandera sea reconocida por un alias corto. `axes` lanzará un error si el usuario proporciona tanto el nombre completo como el alias.
* `map='--nuevo-nombre'`: Reemplaza el nombre de la bandera en la salida. Muy útil para abstraer herramientas subyacentes.
* `map=' '`: Un caso especial. Indica que solo quieres inyectar el **valor** de la bandera, no el nombre de la bandera en sí. Ideal para inyectar valores en posiciones donde no se espera una bandera.

**Ejemplo: Un script `test` que puede pasar una bandera `--marker` a `pytest`.**

```toml
[scripts]
# Usa el pass-through por defecto con un alias.
test = "pytest <params::marker(alias='-m')>"
```

**Ejecución:**

```sh
# Ejecuta todas las pruebas
axes test
# Comando ejecutado: `pytest`

# Ejecuta solo las pruebas marcadas como 'slow'
axes test --marker slow
# Comando ejecutado: `pytest --marker slow`

# Usa el alias
axes test -m smoke
# Comando ejecutado: `pytest -m smoke`
```

**Ejemplo: Un script `deploy` con `map` y `default`.**

```toml
# axes.toml
[scripts]
# El script interno espera --environment, pero exponemos --env al usuario.
# Por defecto, despliega a 'staging'.
deploy = "terraform apply -var 'env=<params::env(map=' ', default='staging')>'"
```

*Observa el uso de `map=' '` para inyectar solo el valor.*

**Ejecución:**

```sh
# Usa el valor por defecto
axes deploy
# Comando ejecutado: terraform apply -var 'env=staging'

# Especifica un entorno
axes deploy --env production
# Comando ejecutado: terraform apply -var 'env=production'
```

### 5.3. El Recolector Genérico: `<params>`

Este es el token "recolector". Es útil cuando quieres pasar un número variable de argumentos o banderas a un comando subyacente sin tener que definirlos todos explícitamente.

* **Sintaxis:** `<params>`
* **Comportamiento:** Se reemplaza por **todos los argumentos** (posicionales y nombrados) que **no fueron consumidos** por un token explícito (`::0`, `::flag`, etc.), manteniendo su orden original.

**Ejemplo: Un `wrapper` genérico para `npm install`.**

```toml
[scripts]
# `add` pasa todos los argumentos restantes a `npm install`,
# pero también maneja explícitamente una bandera `--save-dev` con un alias `-D`.
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

# Instala múltiples dependencias con banderas adicionales
axes add react react-dom --force
# Comando ejecutado: `npm install react react-dom --force`
```

Al combinar estos patrones, puedes construir interfaces de línea de comandos increíblemente ricas y robustas para tus proyectos, todo dentro de la simplicidad de `axes.toml`.

> Para una guía completa con ejemplos detallados de cada tipo de parámetro y modificador, consulta la **[Guía del Sistema de Argumentos (`ARG_PARSER.md`)](./ARG_PARSER.md)**.

## 6. Opciones y Hooks de Entorno

### 6.1. Variables de Entorno `[env]`

Cualquier par clave-valor en `[env]` se inyecta como una variable de entorno en el subproceso del script. Se heredan y pueden ser anuladas.

```toml
[env]
DATABASE_URL = "postgres://user:pass@localhost/db"
APP_ENV = "development"
```

### 6.2. Opciones de Sesión y Herramientas `[options]`

Esta tabla controla el comportamiento de `axes` para sesiones, apertura de proyectos y más.

```toml
[options]
# Especifica el shell a usar para `axes start`. Ej., "bash", "powershell".
shell = "zsh"

# Plantilla para el prompt de la sesión interactiva. Soporta todos los tokens de `axes`.
prompt = "(<#cyan><name><#reset>) 🚀 "

# Un directorio raíz personalizado para todos los archivos de caché binaria. Soporta `~` y vars de entorno.
cache_dir = "~/.axes-caches"
```

#### **Hooks de Sesión: `at_start` y `at_exit`**

Estos son scripts completos de `axes` que se ejecutan automáticamente al entrar (`axes start`) y salir de una sesión interactiva.

```toml
[options]
at_start = { desc = "Activa venv e inicia servicios.", run = [
    "source .venv/bin/activate",
    "docker-compose up -d <params::service(default='db')>"
]}
at_exit = "docker-compose down"
```

#### **Configuración del Comando `open`: `[options.open_with]`**

Define atajos para el comando `axes <ctx> open`. Cada entrada es una **definición de script completa**, permitiendo descripciones y lógica específica de la plataforma.

```toml
[options.open_with]
# Establece la acción por defecto para `axes open`.
default = "editor"

# Cada clave es una `app_key`.
[options.open_with.editor]
desc = "Abre el proyecto en Visual Studio Code."
run = "code \"<path>\""

[options.open_with.terminal]
desc = "Abre un nuevo terminal en la raíz del proyecto."
windows = "wt -d \"<path>\""
default = "gnome-terminal --working-directory=\"<path>\""
```

---

## Conclusión

Ahora tienes una visión completa del archivo `axes.toml`. Al combinar estas características, puedes construir flujos de trabajo potentes, portables y autodocumentados que potenciarán tu productividad de desarrollo.

➡️ **Siguiente Lectura Recomendada: [Referencia Completa de Comandos (`COMMANDS.md`)](./COMMANDS.md)**
