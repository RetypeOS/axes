<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../ARG_PARSER.md">English</a> •
  <a href="./ARG_PARSER.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../ARG_PARSER.md), que es la fuente canónica de la documentación.

# Guía del Sistema de Argumentos: Scripts como Funciones de CLI

El motor de scripts de `axes` te permite hacer mucho más que ejecutar comandos estáticos. Te permite definir **scripts que actúan como funciones de línea de comandos**, aceptando argumentos de manera estructurada, declarativa y validada.

Esta guía explica en profundidad cómo funciona el nuevo y robusto sistema de parseo de `axes` y cómo usar la familia de tokens `<params::...>` para crear flujos de trabajo flexibles y potentes.

## El Paradigma: Pre-Definición y Validación

A diferencia de los scripts de shell tradicionales donde tienes que parsear manualmente `$1` y `$2` (y a menudo de forma frágil), `axes` adopta un paradigma declarativo. Defines los parámetros que tu script espera directamente donde los usas.

Antes de ejecutar una sola línea de tu script, `axes` realiza un análisis completo:

1. **Descubre** todas las definiciones de parámetros (`<params::...>`) en tu script.
2. **Parsea** los argumentos que proporcionaste en la línea de comandos.
3. **Valida** que los argumentos proporcionados coincidan con las definiciones, comprobando requisitos, alias y conflictos.

Solo si esta validación es exitosa, `axes` procede a ensamblar y ejecutar tus comandos. Esto elimina una clase completa de errores y asegura un comportamiento predecible.

> **La Regla de Oro:** Si, al finalizar el análisis, quedan argumentos de CLI sobrantes que no fueron consumidos por ningún token explícito (`<params::0>`, `<params::flag>`, etc.) y el script no incluye el token genérico `<params>`, `axes` lanzará un error para prevenir comportamientos inesperados.

---

## 1. Pre-Parseo de `axes`

Antes de que tus tokens entren en juego, `axes` realiza un simple pre-parseo de los argumentos que pasas en el terminal. Los clasifica en dos tipos:

* **Argumentos Nombrados (Banderas o Flags):** Cualquier token que comience con un guion (`-` o `--`), como `--target` o `-v`. `axes` detecta si una bandera es seguida por un valor (ej., `--target linux`) o si es una bandera booleana sin valor (ej., `--force`).
* **Argumentos Posicionales:** Todos los demás tokens. Se identifican por su posición (0, 1, 2, ...).

Con estos argumentos clasificados, tus definiciones de script pueden comenzar a funcionar.

---

## 2. Parámetros Posicionales

Se accede a los argumentos posicionales por su índice numérico, comenzando en `0`.

### Sintaxis y Modificadores `(...)`

Puedes agregar un bloque de configuración entre paréntesis para refinar el comportamiento de un parámetro.

* **Sintaxis Básica:** `<params::0>`, `<params::1>`, etc.
* **Modificadores:**
  * `required`: La ejecución falla si no se proporciona el argumento.
  * `default='value'`: Proporciona un valor por defecto si el argumento no se pasa en la CLI.
  * `map='--new-flag '`: Transforma el argumento posicional en una bandera con un valor. Si el usuario escribe `comando mi-valor`, y el token es `<params::0(map='--target ')>`, el resultado inyectado será `"--target mi-valor"`.
  * `literal`: Envuelve todo el valor final entre comillas literales, `... "este es un valor posicional" ...`.

#### **Ejemplos (Posicionales)**

**Script para saludar (con `default`):**

```toml
# axes.toml
[scripts]
greet = "echo 'Hola, <params::0(default='Mundo')>!'"
```

```sh
axes . greet          # -> echo 'Hola, Mundo!'
axes . greet Valeria  # -> echo 'Hola, Valeria!'
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
# Hace lint a una ruta, convirtiendo el argumento posicional en una bandera --path.
lint = "eslint <params::0(map='--path ', default='src/')>"
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

Los tokens de parámetro también pueden buscar y consumir banderas (`--nombre`) de la línea de comandos.

### Sintaxis y Comportamiento por Defecto

* **Sintaxis Básica:** `<params::nombre-bandera>`
* **Comportamiento (Pass-through):** Por defecto, un token de bandera busca la bandera correspondiente en la CLI y la reinyecta tal cual, junto con su valor si lo tiene.
  * Si se ejecuta con `--nombre-bandera valor`, el token se expande a `"--nombre-bandera valor"`.
  * Si se ejecuta con `--nombre-bandera` (sin valor), se expande a `"--nombre-bandera"`.
  * Si no se proporciona la bandera, el token se expande a una cadena vacía.

### Modificadores para Banderas `(...)`

* `required`: Falla si la bandera (o su alias) no está presente.
* `default='value'`: Si la bandera se proporciona **sin un valor**, se usará este `default`. También se usa si la bandera **no se proporciona en absoluto**.
* `alias='-a'`: Permite que la bandera sea reconocida por un alias corto. `axes` lanzará un error si el usuario intenta usar ambos (`--nombre-bandera` y `-a`) al mismo tiempo.

* `map='--nuevo-nombre'`: Reemplaza el nombre de la bandera en la salida.

* `map=''` (una cadena vacía): Un caso especial potente. Indica que solo deseas inyectar el **valor** de la bandera, no el nombre de la bandera en sí.

* `literal`: Envuelve todo el valor final entre comillas literales, `--flag "este es un valor de bandera"`.

#### **Ejemplos (Nombrados)**

**Script `build` con modo `release` (Pass-through simple):**

```toml
# axes.toml
[scripts]
build = "cargo build <params::release>"
```

```sh
axes . build            # -> cargo build
axes . build --release  # -> cargo build --release
axes . build --otro-param  # -> Error: Se proporcionaron argumentos inesperados. El script no define un token genérico `<params>` para aceptarlos.
# Argumentos no manejados proporcionados: --otro-param
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
axes . test -m smoke --marker slow # -> Error: Conflicto: Se proporcionaron tanto la bandera '--marker' como su alias '-m'.
```

**Otros posibles casos de uso:**

**Script `copy-file` con múltiples banderas y valores por defecto:**

```toml
# axes.toml
[scripts]
copy = "rsync <params::files-from(alias='-f', default='list.txt')> <params::destination(alias='-d', required)>"
```

```sh
# Usa el valor por defecto para --files-from
axes copy -f --destination ./backup
# -> rsync --files-from list.txt --destination ./backup

# Anula el valor por defecto usando el alias
axes copy -f file.txt -d ./backup
# -> rsync --files-from file.txt --destination ./backup

# Falla si falta el destino requerido
axes copy
# -> Error: La bandera '--destination' es requerida pero no fue proporcionada.
```

**Script `deploy` con `map` y `required`:**

```toml
# axes.toml
[scripts]
# El script interno espera --environment, pero queremos exponer --env al usuario.
deploy = "terraform apply <params::env(map='--environment ', required)>"
```

```sh
axes . deploy --env staging      # -> terraform apply --environment staging
axes . deploy                    # -> Error: La bandera '--env' es requerida pero no fue proporcionada.
```

**Script `docker` con `map=''` para extracción de valor:**
Este es un patrón avanzado para inyectar valores en lugares donde una bandera no es válida.

```toml
# axes.toml
[scripts]
# El tag de la imagen se pasa como una bandera pero se inyecta como un valor posicional.
docker_tag = "docker tag my-image:latest my-org/my-image:<params::tag(map='', default='latest', required)>"
```

```sh
# Ejecución 1: Usa el valor por defecto
axes docker_tag --tag
# Comando ejecutado: `docker tag my-image:latest my-org/my-image:latest`

# Ejecución 2: Especifica el tag
axes docker_tag --tag v1.2.0
# Comando ejecutado: `docker tag my-image:latest my-org/my-image:v1.2.0`
```

---

## 4. El Recolector Genérico: `<params>`

Este es el token "recolector". Es útil cuando quieres pasar un número variable de argumentos o banderas a un comando subyacente sin tener que definirlos todos explícitamente.

* **Sintaxis:** `<params>`
* **Comportamiento:** Se reemplaza por **todos los argumentos** (posicionales y nombrados) que **no fueron consumidos** por un token explícito (`::0`, `::flag`, etc.), manteniendo su orden original.

### **Ejemplo: Un `wrapper` genérico para `cargo run`**

```toml
# axes.toml
[scripts]
# Pasa todos los argumentos no definidos directamente al binario.
run = "cargo run -- <params>"
# Permite una bandera --release opcional, y pasa el resto.
run_release = "cargo run <params::release> -- <params>"
```

```sh
# Ejecución 1: Pasando argumentos al binario (Se usa '/' porque run es un argumento de sistema, es equivalente a `axes ./run ...`)
axes /run --input /data/file.txt --verbose
# Comando ejecutado: `cargo run -- --input /data/file.txt --verbose`

# Ejecución 2: Usando el script con release
axes run_release --input /data/file.txt --release
# `release` es consumido por <params::release> y se expande a `--release`.
# `--input /data/file.txt` es consumido por <params> y se expande a sí mismo.
# Comando ejecutado: `cargo run --release -- --input /data/file.txt`
```

---

## 5. Caso de Uso Avanzado: Construyendo una CLI Componible

`axes` es lo suficientemente potente como para construir interfaces de línea de comandos complejas componiendo variables y parámetros. Este ejemplo crea un script flexible `git-log`.

**Objetivo:** Crear un script `log` que:

1. Use por defecto un formato bonito y de una sola línea.
2. Acepte una bandera opcional `--author="<nombre>"`.
3. Acepte una bandera opcional `--count=<número>` para limitar el número de commits.
4. Acepte una bandera `--stat` para mostrar estadísticas de archivos.

**`axes.toml`:**

```toml
[vars]
# Definimos un formato base que puede ser reutilizado o modificado.
_log_format = "--pretty=format:'%C(yellow)%h %C(cyan)%an %C(green)%s'"

[scripts]
# Nuestro script principal `log` compone todas las piezas.
log = "git log <vars::_log_format> <params::author(map='--author=')> <params::count(alias='-n', map='-n ')> <params::stat>"
```

Esta única línea de `axes.toml` crea un script notablemente potente:

**Cómo funciona:**

-   **`<vars::_log_format>`:** Inyecta la cadena de formato base.
-   **`<params::author(map='--author=')>`:**
    -   Busca una bandera `--author` en la CLI (ej. `axes log --author="John Doe"`).
    -   El `map='--author='` asegura que si se proporciona un valor, se inyecte como `--author=John Doe`. El `=` es crucial aquí. Si no se proporciona la bandera, este token se expande a una cadena vacía.
-   **`<params::count(alias='-n', map='-n ')>`:**
    -   Busca `--count` o `-n` (ej. `axes log -n 5`).
    -   El `map='-n '` asegura que se inyecte como `-n 5` (con un espacio).
-   **`<params::stat>`:**
    -   Esta es una bandera simple de "pass-through". Si se ejecuta `axes log --stat`, este token se expande a `--stat`. De lo contrario, es una cadena vacía.

**Ejecuciones de Ejemplo:**

```sh
# Log simple con el formato bonito por defecto
$ axes log
# -> git log --pretty=format:'%C(yellow)%h %C(cyan)%an %C(green)%s'

# Limitar a 5 commits y mostrar estadísticas
$ axes log -n 5 --stat
# -> git log --pretty=format:'%C(yellow)%h %C(cyan)%an %C(green)%s' -n 5 --stat

# Filtrar por autor
$ axes log --author="Jane Doe"
# -> git log --pretty=format:'%C(yellow)%h %C(cyan)%an %C(green)%s' --author="Jane Doe"
```

Esto demuestra cómo `axes` puede construir un wrapper de CLI sofisticado y validado alrededor de cualquier herramienta existente con solo unas pocas líneas de configuración declarativa, eliminando por completo la necesidad de scripts de shell complejos.

➡️ **Para un ejemplo real y avanzado, mira cómo `axes` usa su propio motor para ejecutar su [suite de benchmarking](./examples/stress_tests/.axes/axes.toml).**

---

Al combinar estos patrones, puedes construir y/o modificar interfaces de línea de comandos para tus scripts que son tan potentes, legibles y seguras como las de cualquier herramienta nativa, todo desde la simplicidad de tu `axes.toml`.
