# Guía del Sistema de Argumentos: Scripts como Funciones

El motor de scripting de `axes` te permite hacer mucho más que ejecutar comandos estáticos. Te permite definir **scripts que actúan como funciones de línea de comandos**, aceptando argumentos de forma estructurada y declarativa.

Esta guía explica en profundidad cómo funciona el sistema de parseo de `axes` y cómo usar la familia de tokens `<axes::params::...>` para crear flujos de trabajo flexibles y potentes.

## El Paradigma: Definición de Parámetros en Línea

A diferencia de los scripts de shell tradicionales donde tienes que parsear `$1`, `$2` manualmente, `axes` te permite definir cómo se deben tratar los parámetros directamente en el lugar donde los usas.

Cada token `<axes::params::...>` no solo consume un argumento de la CLI, sino que también puede definir reglas sobre él, como su valor por defecto, si es obligatorio o si tiene alias, usando una sintaxis similar a una función: `nombre(opción1='valor', opción2)`.

> **La Regla de Oro:** Si un script se invoca con argumentos, su definición en `axes.toml` **debe** contener al menos un token `<axes::params::...>` para indicar cómo deben usarse. Si al final de la expansión sobran argumentos que no fueron consumidos por ningún token, `axes` lanzará un error.

---

## 1. El Pre-Parseo de `axes`

Antes de que tus tokens entren en acción, `axes` realiza un pre-parseo simple de los argumentos que le pasas en la terminal. Los clasifica en dos tipos:

* **Argumentos Nombrados (Flags):** Cualquier token que empiece con un guion (`-` o `--`), como `--target` o `-v`. `axes` también detecta si un flag va seguido de un valor (ej. `--target linux`) o si es un flag booleano (ej. `--force`).
* **Argumentos Posicionales:** Todos los demás tokens. Se identifican por su posición (0, 1, 2, ...).

Con estos argumentos clasificados, tus tokens en `axes.toml` pueden empezar a trabajar.

---

## 2. Parámetros Posicionales

Se accede a los argumentos posicionales por su índice numérico.

* **Sintaxis Básica:** `<axes::params::0>`, `<axes::params::1>`, etc.
* **Comportamiento:** Se reemplaza por el argumento posicional en ese índice. Si el argumento no se proporcionó, el token se expande a una cadena vacía.

### Modificadores de Configuración: `(...)`

Puedes añadir un bloque de configuración entre paréntesis para refinar el comportamiento de un parámetro. Las opciones disponibles son:

* `default='valor'`: Proporciona un valor por defecto si el argumento no se pasa en la CLI.
* `required`: Hace que la ejecución falle si el argumento no se proporciona.
* `map='--nuevo-flag'`: Permite que un argumento posicional sea tratado como un flag. Si se pasa `--nuevo-flag`, el valor de ese flag se usará para este parámetro posicional.

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
axes . create_file               # -> Error: El parámetro posicional '0' es requerido.
```

**Script de `lint` (con `map`):**

```toml
# axes.toml
[scripts]
# Por defecto, hace linting del directorio `src`.
# Pero permite tratar un argumento posicional como un flag `--path`.
lint = "eslint <axes::params::0(map='--path', default='src/')>"
lint_req = "eslint <axes::params::0(map='--path')>"
```

```sh
# Ejecución 1: Usa el valor por defecto
axes . lint
# Comando ejecutado: `eslint --path src/`

# Ejecución 2: Usa el primer argumento para cambiar la ruta como un flag
axes . lint tests/
# Comando ejecutado: `eslint --path tests/`

# Ejecución 3: flag opcional:
axes . lint_req
# Comando ejecutado: `eslint`

# Ejecución 4: Usa el primer argumento para pasar un flag opcional dependiendo si existe o no el argumento.
axes . lint tests/
# Comando ejecutado: `eslint --path tests/`

```

## 3. Parámetros Nombrados (Flags)

Los tokens de parámetros también pueden buscar y consumir flags (`--nombre`) de la línea de comandos. Esto te permite crear scripts con interfaces complejas y legibles.

* **Sintaxis Básica:** `<axes::params::nombre_flag>`
* **Comportamiento:** Busca el flag `--nombre_flag` en la CLI.
  * Si se encuentra como `--nombre_flag valor`, el token se expande a `--nombre_flag valor`.
  * Si se encuentra como `--nombre_flag` (sin valor), se expande a `--nombre_flag`.
  * Si no se encuentra, se expande a una cadena vacía.

### Modificadores de Configuración para Flags

Los flags también aceptan modificadores en un bloque `(...)` para un control total.

* `alias='-a'`: Permite que el flag sea reconocido por su nombre completo (`--nombre_flag`) o por un alias corto (`-a`).
* `map='--valor-final'`: Cambia a qué se expande el token. Si se encuentra el flag `--nombre_flag`, el token se reemplazará por `'--valor-final'` en lugar de por el flag original.
* `default='valor'`: Si no se proporciona un valor para el flag pero se llama, el token se expandirá a este valor por defecto.
* `required`: La ejecución fallará si el flag (`--nombre_flag` o su alias) no está presente en la línea de comandos.

#### **Ejemplos (Nombrados)**

**Script de `build` con modo `release`:**
Este es el patrón más común para flags booleanos.

```toml
# axes.toml
[scripts]
# Si se pasa `--release`, inserta '--release' en el comando.
build = "cargo build <axes::params::release>"
```

```sh
axes . build            # -> cargo build
axes . build --release  # -> cargo build --release
axes . build --otro-argumento # -> Error, hay argumentos no definidos: ['--otro-argumento']
```

**Script de `test` con alias y valor:**

```toml
# axes.toml
[scripts]
# Pasa el flag `--marker` o `-m` a pytest.
test = "pytest <axes::params::marker(alias='-m')>"
```

```sh
axes . test --marker slow   # -> pytest --marker slow
axes . test -m smoke        # -> pytest --marker smoke
```

**Script de `deploy` con `default` y `required`:**

```toml
# axes.toml
[scripts]
# Requiere un entorno, pero por defecto es 'staging'.
deploy = "terraform apply -var 'env=<axes::params::env(map='', default='staging', required)>'"
# Una forma más sencilla de definir esto sería con un argumento posicional, pero el ejemplo muestra el poder de uso de incluso usar flags como argumentos posicionales si se desea para tu propia estructura.
```

```sh
# Ejecución 1: Usa el default
axes . deploy
# -> terraform apply -var 'env=staging'

# Ejecución 2: Especifica el entorno
axes . deploy --env production
# -> terraform apply -var 'env=production'

# Ejecución 3: Intenta ejecutar sin el flag (si no tuviera default)
# Error: El parámetro '--env' es requerido por el script 'deploy'.
```

Combinando estos patrones, puedes construir interfaces de línea de comandos para tus scripts que son tan potentes como las de cualquier herramienta nativa.
