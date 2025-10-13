<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../ARG_PARSER.md">English</a> •
  <a href="./ARG_PARSER.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../ARG_PARSER.md), que es la fuente canónica de la documentación.

# Guía del Sistema de Argumentos: Scripts como Funciones CLI

El motor de scripting de `axes` te permite hacer mucho más que ejecutar comandos estáticos. Te permite definir **scripts que actúan como funciones de línea de comandos**, aceptando argumentos de manera estructurada, declarativa y validada.

Esta guía explica en profundidad cómo funciona el nuevo y robusto sistema de *parsing* de `axes` y cómo utilizar la familia de tokens `<params::...>` para crear flujos de trabajo flexibles y potentes.

## El Paradigma: Predefinición y Validación

A diferencia de los scripts de shell tradicionales donde tienes que hacer *parsing* manual de `$1` y `$2` (y a menudo de forma frágil), `axes` adopta un paradigma declarativo. Defines los parámetros que tu script espera directamente donde los usas.

Antes de ejecutar una sola línea de tu script, `axes` realiza un análisis completo:

1. **Descubre** todas las definiciones de parámetros (`<params::...>`) en tu script.
2. **Parsea** los argumentos que proporcionaste en la línea de comandos.
3. **Valida** que los argumentos proporcionados coincidan con las definiciones, comprobando requisitos, alias y conflictos.

Solo si esta validación es exitosa, `axes` procede a ensamblar y ejecutar tus comandos. Esto elimina toda una clase de errores y asegura un comportamiento predecible.

> **La Regla de Oro:** Si, al finalizar el análisis, quedan argumentos de la CLI que no fueron consumidos por ningún token explícito (`<params::0>`, `<params::flag>`, etc.) y el script no incluye el token genérico `<params>`, `axes` lanzará un error para prevenir un comportamiento inesperado.

---

## 1. Pre-Parseo de `axes`

Antes de que tus tokens entren en juego, `axes` realiza un simple pre-parseo de los argumentos que pasas en la terminal. Clasifica estos argumentos en dos tipos:

* **Argumentos Nombrados (Flags):** Cualquier token que comience con un guion (`-` o `--`), como `--target` o `-v`. `axes` detecta si un flag está seguido de un valor (ej. `--target linux`) o si es un flag booleano sin valor (ej. `--force`).
* **Argumentos Posicionales:** Todos los demás tokens. Se identifican por su posición (0, 1, 2, ...).

Con estos argumentos clasificados, las definiciones de tu script pueden empezar a operar.

---

## 2. Parámetros Posicionales

Los argumentos posicionales se acceden por su índice numérico, comenzando en `0`.

### Sintaxis y Modificadores `(...)`

Puedes añadir un bloque de configuración entre paréntesis para refinar el comportamiento de un parámetro.

* **Sintaxis Básica:** `<params::0>`, `<params::1>`, etc.
* **Modificadores:**
  * `required`: La ejecución falla si no se proporciona el argumento.
  * `default='value'`: Proporciona un valor por defecto si el argumento no se pasa en la CLI.
  * `map='--new-flag'`: Transforma el argumento posicional en un flag con valor. Si el usuario escribe `command my-value`, y el token es `<params::0(map='--target ')>`, el resultado inyectado será `"--target my-value"`.

**Notese que map inserta de forma literal, por lo que si quiere separar clave de valor, debe poner un espacio(' ') al final del valor de map.**

#### **Ejemplos (Posicionales)**

**Script para saludar (con `default`):**

```toml
# axes.toml
[scripts]
greet = "echo 'Hello, <params::0(default='World')>!'"
```

```sh
axes . greet          # -> echo 'Hello, World!'
axes . greet Valeria  # -> echo 'Hello, Valeria!'
```

**Script para crear un archivo (con `required`):**

```toml
# axes.toml
[scripts]
create_file = "touch <params::0(required)>"
```

```sh
axes . create_file src/index.js  # -> touch src/index.js
axes . create_file               # -> Error: El argumento posicional en el índice 0 es requerido pero no fue proporcionado.
```

**Script `lint` (con `map`):**
Este patrón es extremadamente útil para crear interfaces más legibles.

```toml
# axes.toml
[scripts]
# Hace lint al path, convirtiendo el argumento posicional en un flag --path.
lint = "eslint <params::0(map='--path ', default='src/')>"
```

```sh
# Ejecución 1: Usa el valor por defecto
axes . lint
# Comando ejecutado: `eslint --path src/`

# Ejecución 2: Especifica un path
axes . lint tests/
# Comando ejecutado: `eslint --path tests/`
```

---

## 3. Parámetros Nombrados (Flags)

Los tokens de parámetros también pueden buscar y consumir flags (`--nombre`) desde la línea de comandos.

### Sintaxis y Comportamiento por Defecto

* **Sintaxis Básica:** `<params::flag-name>`
* **Comportamiento (Pase-a-través):** Por defecto, un token de flag busca el flag correspondiente en la CLI y lo reinjecta tal cual, junto con su valor si lo tiene.
  * Si se ejecuta con `--flag-name value`, el token se expande a `"--flag-name value"`.
  * Si se ejecuta con `--flag-name` (sin valor), se expande a `"--flag-name"`.
  * Si el flag no se proporciona, el token se expande a una cadena vacía.

### Modificadores para Flags `(...)`

* `required`: Falla si el flag (o su alias) no está presente.
* `default='value'`: Si el flag se proporciona **sin valor**, se usará este `default`. También se usa si el flag **no se proporciona en absoluto**.
* `alias='-a'`: Permite que el flag sea reconocido por un alias corto. `axes` lanzará un error si el usuario intenta usar ambos (`--flag-name` y `-a`) al mismo tiempo.
* `map='--new-name'`: Reemplaza el nombre del flag en la salida.
* `map=' '`: Un caso especial muy potente. Indica que solo quieres inyectar el **valor** del flag, no el flag en sí.

#### **Ejemplos (Nombrados)**

**Script `build` con modo `release` (Pase-a-través Simple):**

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
# El script interno espera --environment, pero queremos exponer --env al usuario.
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

---

## 4. El Colector Genérico: `<params>`

Este es el token "colector". Es útil cuando quieres pasar un número variable de argumentos o flags a un comando subyacente sin tener que definirlos todos explícitamente.

* **Sintaxis:** `<params>`
* **Comportamiento:** Se reemplaza por **todos los argumentos** (posicionales y nombrados) que **no fueron consumidos** por un token explícito (`::0`, `::flag`, etc.), manteniendo su orden original.

### **Ejemplo: Un `wrapper` genérico para `cargo run`**

```toml
# axes.toml
[scripts]
# Pasa todos los argumentos no definidos directamente al binario.
run = "cargo run -- <params>"
# Permite un flag opcional --release, y pasa el resto.
run_release = "cargo run <params::release> -- <params>"
```

```sh
# Ejecución 1: Pasando argumentos al binario (Usa '/' porque run es un argumento de sistema, equivalente a `axes ./run ...`)
axes /run --input /data/file.txt --verbose
# Comando ejecutado: `cargo run -- --input /data/file.txt --verbose`

# Ejecución 2: Usando el script con release
axes run_release --input /data/file.txt --release
# `release` es consumido por <params::release> y se expande a `--release`.
# `--input /data/file.txt` es consumido por <params> y se expande a sí mismo.
# Comando ejecutado: `cargo run --release -- --input /data/file.txt`
```

Al combinar estos patrones, puedes construir y/o modificar interfaces de línea de comandos para tus scripts que sean tan potentes y legibles como las de cualquier herramienta nativa, todo desde la sencillez de tu `axes.toml`.
