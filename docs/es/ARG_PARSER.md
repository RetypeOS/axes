<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../ARG_PARSER.md">English</a> •
  <a href="./ARG_PARSER.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../ARG_PARSER.md), que es la fuente canónica de la documentación.

# Guía del Sistema de Argumentos: Scripts como Funciones de CLI

El motor de scripting de `axes` te permite hacer mucho más que ejecutar comandos estáticos. Te permite definir **scripts que actúan como funciones de línea de comandos**, aceptando argumentos de forma estructurada, declarativa y validada.

Esta guía explica en profundidad cómo funciona el nuevo y robusto sistema de parseo de `axes` y cómo usar la familia de tokens `<axes::params::...>` para crear flujos de trabajo flexibles y potentes.

## El Paradigma: Definición y Validación Previas

A diferencia de los scripts de shell tradicionales donde tienes que parsear `$1` y `$2` manualmente (y a menudo de forma frágil), `axes` adopta un paradigma declarativo. Defines los parámetros que tu script espera directamente en el lugar donde los usas.

Antes de ejecutar una sola línea de tu script, `axes` realiza un análisis completo:

1. **Descubre** todas las definiciones de parámetros (`<axes::params::...>`) en tu script.
2. **Parsea** los argumentos que proporcionaste en la línea de comandos.
3. **Valida** que los argumentos proporcionados coincidan con las definiciones, comprobando requisitos, alias y conflictos.

Solo si esta validación es exitosa, `axes` procede a ensamblar y ejecutar tus comandos. Esto elimina toda una clase de errores y garantiza un comportamiento predecible.

> **La Regla de Oro:** Si al final del análisis sobran argumentos de la CLI que no fueron consumidos por ningún token explícito (`<axes::params::0>`, `<axes::params::flag>`, etc.) y el script no incluye el token genérico `<axes::params>`, `axes` lanzará un error para prevenir un comportamiento inesperado.

---

## 1. El Pre-Parseo de `axes`

Antes de que tus tokens entren en acción, `axes` realiza un pre-parseo simple de los argumentos que le pasas en la terminal. Los clasifica en dos tipos:

* **Argumentos Nombrados (Flags):** Cualquier token que empiece con un guion (`-` o `--`), como `--target` o `-v`. `axes` detecta si un flag va seguido de un valor (ej. `--target linux`) o si es un flag booleano sin valor (ej. `--force`).
* **Argumentos Posicionales:** Todos los demás tokens. Se identifican por su posición (0, 1, 2, ...).

Con estos argumentos clasificados, las definiciones de tu script pueden empezar a trabajar.

---

## 2. Parámetros Posicionales

Se accede a los argumentos posicionales por su índice numérico, empezando en `0`.

### Sintaxis y Modificadores `(...)`

Puedes añadir un bloque de configuración entre paréntesis para refinar el comportamiento de un parámetro.

* **Sintaxis Básica:** `<axes::params::0>`, `<axes::params::1>`, etc.
* **Modificadores:**
  * `required`: La ejecución falla si el argumento no se proporciona.
  * `default='valor'`: Proporciona un valor por defecto si el argumento no se pasa en la CLI.
  * `map='--nuevo-flag'`: Transforma el argumento posicional en un flag con valor. Si el usuario escribe `comando mi-valor`, y el token es `<axes::params::0(map='--target')>`, el resultado inyectado será `"--target mi-valor"`.

#### **Ejemplos (Posicionales)**

**Script para saludar (con `default`):**

```toml
# axes.toml
[scripts]
greet = "echo 'Hola, <axes::params::0(default='Mundo')>!'"
```

```sh
axes . greet          # -> echo 'Hola, Mundo!'
axes . greet Valeria  # -> echo 'Hola, Valeria!'
```

**Script para crear un fichero (con `required`):**

```toml
# axes.toml
[scripts]
create_file = "touch <axes::params::0(required)>"
```

```sh
axes . create_file src/index.js  # -> touch src/index.js
axes . create_file               # -> Error: Positional argument at index 0 is required but was not provided.
```

**Script de `lint` (con `map`):**
Este patrón es extremadamente útil para crear interfaces más legibles.

```toml
# axes.toml
[scripts]
# Hace linting de una ruta, convirtiendo el argumento posicional en un flag --path.
lint = "eslint <axes::params::0(map='--path', default='src/')>"
```

```sh
# Ejecución 1: Usa el valor por defecto
axes . lint
# Comando ejecutado: `eslint --path src/`

# Ejecución 2: Especifica una ruta
axes . lint tests/
# Comando ejecutado: `eslint --path tests/`
```

---

## 3. Parámetros Nombrados (Flags)

Los tokens de parámetros también pueden buscar y consumir flags (`--nombre`) de la línea de comandos.

### Sintaxis y Comportamiento por Defecto

* **Sintaxis Básica:** `<axes::params::nombre-flag>`
* **Comportamiento (Pass-through):** Por defecto, un token de flag busca el flag correspondiente en la CLI y lo reinyecta tal cual, junto con su valor si lo tiene.
  * Si se ejecuta con `--nombre-flag valor`, el token se expande a `"--nombre-flag valor"`.
  * Si se ejecuta con `--nombre-flag` (sin valor), se expande a `"--nombre-flag"`.
  * Si el flag no se proporciona, el token se expande a una cadena vacía.

### Modificadores para Flags `(...)`

* `required`: Falla si el flag (o su alias) no está presente.
* `default='valor'`: Si el flag se proporciona **sin un valor**, se usará este `default`. También se usa si el flag **no se proporciona en absoluto**.
* `alias='-a'`: Permite que el flag sea reconocido por un alias corto. `axes` lanzará un error si el usuario intenta usar ambos (`--nombre-flag` y `-a`) al mismo tiempo.
* `map='--nuevo-nombre'`: Reemplaza el nombre del flag en la salida.
* `map=' '`: Un caso especial muy potente. Indica que solo quieres inyectar el **valor** del flag, no el flag en sí.

#### **Ejemplos (Nombrados)**

**Script de `build` con modo `release` (Pass-through simple):**

```toml
# axes.toml
[scripts]
build = "cargo build <axes::params::release>"
```

```sh
axes . build            # -> cargo build
axes . build --release  # -> cargo build --release
```

**Script de `test` con alias:**

```toml
# axes.toml
[scripts]
test = "pytest <axes::params::marker(alias='-m')>"
```

```sh
axes . test --marker slow   # -> pytest --marker slow
axes . test -m smoke        # -> pytest --marker smoke
axes . test -m smoke --marker slow # -> Error: Conflict: Both flag '--marker' and its alias '-m' were provided.
```

**Script de `deploy` con `map` y `required`:**

```toml
# axes.toml
[scripts]
# El script interno espera --environment, pero queremos exponer --env al usuario.
deploy = "terraform apply <axes::params::env(map='--environment', required)>"
```

```sh
axes . deploy --env staging      # -> terraform apply --environment staging
axes . deploy                    # -> Error: Flag '--env' is required but was not provided.
```

**Script de `docker` con `map=''` para extracción de valor:**
Este es un patrón avanzado para inyectar valores en lugares donde un flag no es válido.

```toml
# axes.toml
[scripts]
# El tag de la imagen se pasa como un flag, pero se inyecta como un valor posicional.
docker_tag = "docker tag mi-imagen:latest mi-org/mi-imagen:<axes::params::tag(map='', default='latest')>"
```

```sh
# Ejecución 1: Usa el default
axes . docker_tag
# Comando ejecutado: `docker tag mi-imagen:latest mi-org/mi-imagen:latest`

# Ejecución 2: Especifica el tag
axes . docker_tag --tag v1.2.0
# Comando ejecutado: `docker tag mi-imagen:latest mi-org/mi-imagen:v1.2.0`
```

---

## 4. El Recolector Genérico: `<axes::params>`

Este es el token "recolector". Es útil cuando quieres pasar un número variable de argumentos o flags a un comando subyacente sin tener que definirlos todos explícitamente.

* **Sintaxis:** `<axes::params>`
* **Comportamiento:** Se reemplaza por **todos los argumentos** (posicionales y nombrados) que **no fueron consumidos** por un token explícito (`::0`, `::flag`, etc.), manteniendo su orden original.

### **Ejemplo: Un `wrapper` genérico para `cargo run`**

```toml
# axes.toml
[scripts]
# Pasa todos los argumentos no definidos directamente al binario.
run = "cargo run -- <axes::params>"
# Permite un flag --release opcional, y pasa el resto.
run_release = "cargo run <axes::params::release> -- <axes::params>"
```

```sh
# Ejecución 1: Pasar argumentos al binario
axes . run --input /data/file.txt --verbose
# Comando ejecutado: `cargo run -- --input /data/file.txt --verbose`

# Ejecución 2: Usar el script con release
axes . run_release --input /data/file.txt --release
# `release` es consumido por <axes::params::release> y se expande a `--release`.
# `--input /data/file.txt` es consumido por <axes::params> y se expande a sí mismo.
# Comando ejecutado: `cargo run --release -- --input /data/file.txt`
```

Combinando estos patrones, puedes construir y/o modificar interfaces de línea de comandos para tus scripts que son tan potentes, legibles y seguras como las de cualquier herramienta nativa, todo desde la simplicidad de tu `axes.toml`.
