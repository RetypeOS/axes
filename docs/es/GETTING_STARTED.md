<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../GETTING_STARTED.md">English</a> •
  <a href="./GETTING_STARTED.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../GETTING_STARTED.md), que es la fuente canónica de la documentación.

# Guía de Inicio Rápido: Tu Primer Monorepo Orquestado con `axes`

¡Bienvenido a `axes`! Esta guía te llevará de cero a un monorepo completamente funcional y orquestado. En los próximos 15-20 minutos, aprenderás a:

* ✅ Instalar `axes` en tu sistema.
* ✅ Crear tu primer proyecto y subproyectos.
* ✅ Definir y ejecutar scripts utilizando la nueva gramática universal.
* ✅ Aprovechar la herencia de variables entre proyectos.
* ✅ Orquestar un flujo de trabajo complejo que involucra múltiples proyectos.
* ✅ Utilizar sesiones de proyecto para un flujo de trabajo enfocado.

Al final de este tutorial, comprenderás el poder fundamental de `axes` y estarás listo para aplicarlo a tus propios proyectos.

---

## 1. Instalación

`axes` es un único archivo binario sin dependencias externas, lo que hace que su instalación sea muy sencilla.

### Opción A: Descargar el Binario Precompilado (Recomendada)

Esta es la forma más rápida de empezar.

1. **Ve a la Página de Releases:** Abre la [página oficial de Releases de `axes` en GitHub](https://github.com/RetypeOS/axes/releases).
2. **Descarga el archivo correcto:** Busca la última versión y descarga el archivo `.zip` o `.tar.gz` que corresponda a tu sistema operativo (Windows, macOS o Linux).
3. **Descomprime el archivo:** Dentro, encontrarás un único ejecutable: `axes.exe` (en Windows) o `axes` (en macOS/Linux).
4. **Mueve el ejecutable a tu `PATH`:** Este es el paso más importante. Para poder llamar a `axes` desde cualquier lugar de tu terminal, debes mover este archivo a un directorio que esté en la variable de entorno `PATH` de tu sistema.

    * **Windows:**
        1. Crea una carpeta, por ejemplo, `C:\Program Files\axes`.
        2. Mueve `axes.exe` a esa carpeta.
        3. Busca "Editar las variables de entorno del sistema" en el menú de inicio, abre el editor de `PATH` y añade la ruta `C:\Program Files\axes` a la lista.
    * **macOS / Linux:**
        Un directorio común y recomendado es `/usr/local/bin`. Puedes mover el archivo con este comando en tu terminal (puede que necesites `sudo`):

        ```sh
        sudo mv ./axes /usr/local/bin/axes
        ```

5. **Verifica la instalación:** Abre una **nueva** ventana de terminal (esto es importante para que se carguen los cambios de `PATH`) y ejecuta:

    ```sh
    axes --version
    ```

    Si ves un número de versión, ¡la instalación fue exitosa!

### Opción B: Compilar desde el Código Fuente

Si tienes la [cadena de herramientas de Rust](https://www.rust-lang.org/tools/install) instalada, puedes compilar `axes` tú mismo.

```sh
# 1. Clona el repositorio
git clone https://github.com/RetypeOS/axes.git

# 2. Navega al directorio
cd axes

# 3. Compila en modo release (optimizado)
cargo build --release
```

El ejecutable final se encontrará en `./target/release/axes`. Puedes mover este archivo a tu `PATH` como se describe en la Opción A.

---

Con `axes` instalado, estás listo para crear tu primer proyecto. ¡Vamos a ello!

## 2. Nuestro Escenario y Navegación de Contexto

Para este tutorial, construiremos la estructura de un sitio web corporativo ficticio llamado "Innovatech". Este sitio tendrá dos componentes principales: un **blog** y una **tienda en línea**.

Antes de comenzar, es crucial entender cómo `axes` se refiere a los proyectos. Al igual que navegas por un sistema de archivos con `cd`, `axes` navega por su árbol lógico de proyectos utilizando **contextos**. Estos se utilizan para decirle a comandos como `info`, `tree` o `start` sobre qué proyecto operar.

| Contexto | Descripción                                                                    | Ejemplo (desde `.../innovatech-website/blog`) |
| :------- | :----------------------------------------------------------------------------- | :----------------------------------- |
| `nombre` | Un hijo directo del proyecto raíz (`global`).                                   | `axes innovatech-website info`       |
| `/`      | El separador de jerarquía.                                                     | `axes innovatech-website/blog info`  |
| `.`      | El proyecto más cercano encontrado en el directorio actual o cualquier directorio padre. | `axes . info` (resuelve a `innovatech-website/blog`)    |
| `_`      | **(Efímero)** El proyecto en el directorio actual, ejecutado sin usar el índice global. | `axes _ info` (compila `axes.toml` solo en memoria) |
| `..`     | El padre del proyecto actual (desde la sesión o CWD).                          | `axes .. info` (resuelve a `innovatech-website`)  |
| `**`     | El último proyecto utilizado en cualquier parte del sistema.                     | `axes ** start`    |
| `*`      | El último hijo usado del proyecto padre actual.                                | `axes innovatech-website/* start`    |
| `alias!` | Un atajo personalizado que creas.                                               | `axes blog! info`  |

A lo largo de este tutorial, usaremos estos contextos para que veas lo fluidos y potentes que son.

### Creando el Proyecto Contenedor

Primero, crea un directorio para todo el monorepo y, dentro de él, inicializa tu proyecto raíz de `axes`.

```sh
mkdir innovatech-website && cd innovatech-website
axes init
```

Acepta los valores por defecto en el asistente interactivo (nombre: `innovatech-website`, padre: `global`, etc.). Ahora, personaliza el `axes.toml` generado para que sea la base de nuestro monorepo:

```toml
# ./innovatech-website/.axes/axes.toml
version = "1.0.0"
description = "El monorepo para el sitio web de Innovatech."

[vars]
company_name = "Innovatech Inc."

[scripts]
check_copyright = "echo \"© $(date +%Y) <vars::company_name>. Todos los derechos reservados.\""
```

Hemos definido una variable y un script que actuarán como configuración compartida para todo nuestro monorepo.

---

## 3. El Primer Subproyecto: El Blog

Ahora, vamos a crear el blog como **hijo** de `innovatech-website`.

```sh
# Dentro de innovatech-website/, crea y entra al directorio blog
mkdir blog && cd blog

# Inicializa `axes`, usando `..` para referirte al padre (`innovatech-website`)
axes init --parent ..
```

En el asistente, `axes` interpretará `..` como el proyecto en el directorio padre o superior y te lo sugerirá. ¡Ya estás usando la navegación por contexto!

Para visualizar la nueva estructura, vuelve al directorio padre y ejecuta `tree`:

```sh
# Desde el directorio innovatech-website/
cd ..
axes tree # Implícito por '.'

# O, más inteligentemente, desde dentro de `blog/`:
# "Muéstrame el árbol de mi padre"
axes .. tree
```

Ambos mostrarán:

```text
innovatech-website
└─ blog
```

### Demostrando la Herencia

Ahora, abre el `axes.toml` dentro de `blog/` y define un script que utilice la configuración heredada:

```toml
# ./innovatech-website/blog/.axes/axes.toml
version = "0.1.0"
description = "El blog de Innovatech."

[scripts]
build = "hugo --minify"
# Este script COMPONE el script 'check_copyright' heredado del padre.
generate_footer = [
    "# --- Generando Pie de Página del Blog ---",
    "<scripts::check_copyright>",
    "# Construido con <name>"
]
```

Para ejecutar un script en tu proyecto actual, simplemente usa su nombre.

```sh
# Estando en el directorio blog/
axes generate_footer
```

La salida será:

```text
 --- Generando Pie de Página del Blog ---
© 2025 Innovatech Inc.. Todos los derechos reservados.
 Construido con innovatech-website/blog
```

Has compartido configuración y lógica de forma limpia y has navegado por tu proyecto de forma intuitiva. A continuación, añadiremos más complejidad con nuestra tienda en línea.

## 4. El Segundo Subproyecto: La Tienda en Línea

Nuestra tienda en línea será el tercer proyecto en nuestro árbol. El proceso es idéntico al del blog.

```sh
# Desde el directorio raíz (innovatech-website/)
mkdir store && cd store

# Inicializa, de nuevo especificando el padre con `..`
axes init --name store --parent ..
```

Después del asistente, tu árbol de proyectos (`axes innovatech-website tree`) se verá así:

```text
innovatech-website
├─ blog
└─ store
```

Ahora, vamos a darle a la tienda un script más avanzado. Edita el nuevo `axes.toml` en `store/`:

```toml
# ./innovatech-website/store/.axes/axes.toml
version = "1.0.0"
description = "La tienda en línea de Innovatech."

[scripts]
# Este script de prueba acepta un parámetro posicional.
# `<params::0>` será reemplazado por el primer argumento
# que pasemos al script desde la línea de comandos.
test_module = "pytest tests/test_<params::0>.py"
```

Para ejecutar un script en un proyecto **diferente**, utilizarás el comando `run` explícitamente o `/nombre_script`. Esto elimina la ambigüedad y deja clara tu intención. [Esto se cambiará quizás para obtener una sintaxis más robusta]

```sh
# Desde el directorio innovatech-website/
axes store run test_module payments  # --> ejecutará `pytest tests/test_payments.py` (Modo Explícito con `run`)
# O su atajo
axes store/test_module products  # --> ejecutará `pytest tests/test_products.py` (Modo Implícito con <ctx>/script_name [args...])
```

Has creado un atajo reutilizable y parametrizable, eliminando la necesidad de recordar o escribir rutas de prueba largas y complejas.

> **Profundiza:** El sistema de parámetros de `axes` es extremadamente potente, permitiendo banderas, valores por defecto y más. Para dominarlo, consulta nuestra guía completa: **[Dominando `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.

### Una Mirada más Profunda: La Gramática Universal de Comandos

El comando `axes store/test_module payments` es un ejemplo primordial de la característica más potente y ergonómica de `axes`: su **gramática universal de comandos**.

A diferencia de los ejecutores de tareas tradicionales que a menudo requieren un subcomando específico como `run` para ejecutar un script (`ejecutor_tareas run <script> -- <args>`), `axes` trata la combinación de `<contexto>/<nombre_script>` como un único comando, directamente ejecutable.

Esto crea un **"espacio de comandos virtual"** donde cada script en todo tu monorepo se vuelve accesible como si fuera un binario nativo en tu `PATH`.

#### Cómo Funciona

El despachador de `axes` está diseñado para analizar inteligentemente esta gramática. Cuando recibe un comando, divide el primer argumento en el *último* `/` que encuentra:

```sh
axes store/test_module payments
#    └─┬─┘ └───┬─────┘ └───┬───···>
#      │       │           └─ Parámetros (pasados al script)
#      │       └─ Nombre del Script
#      └─ Contexto
```

* Todo lo anterior al último `/` (`store`) se trata como el **contexto**.
* Todo lo posterior (`test_module`) es el **nombre del script** a ejecutar dentro de ese contexto.
* Cualquier argumento posterior (`payments`) se pasa como **parámetros** al script.

Esta elección de diseño es intencional. Hace que los scripts de tu proyecto se sientan como comandos nativos de primera clase, reduciendo la carga cognitiva y haciendo que tu flujo de trabajo se sienta fluido e integrado con tu shell. Esto convierte todo tu monorepo en una única aplicación de línea de comandos cohesionada.

---

## 5. Orquestación Maestra

Hemos creado proyectos individuales, cada uno con sus propios scripts. Ahora, vamos a unirlos. El verdadero poder de `axes` reside en su capacidad para actuar como el director de orquesta de todo tu ecosistema.

Volvamos al `axes.toml` del proyecto padre, `innovatech-website`, para crear flujos de trabajo que controlen a los hijos.

```toml
# ./innovatech-website/.axes/axes.toml

version = "1.0.0"
description = "El monorepo para el sitio web de Innovatech."

[vars]
company_name = "Innovatech Inc."

[scripts]
check_copyright = "echo \"© $(date +%Y) <vars::company_name>. Todos los derechos reservados.\""

# Un script que llama a los scripts de construcción de sus hijos en paralelo.
build_all = [
    "# 🚀 Construyendo todo el sitio web en paralelo...",
    "@> axes blog/build",
    "@> axes store/build" # Asumiendo que `store` también tiene un script `build`.
]

# Un script de calidad que ejecuta comprobaciones en secuencia.
quality = [
    "#  Comprobaciones del Linter...",
    "axes blog/lint",
    "axes store/lint",
    "# Ejecutando pruebas unitarias...",
    "axes blog/test",
    "axes store/test"
]
```

Con esta configuración, has creado puntos de entrada únicos para operaciones complejas en todo el monorepo:

```sh
# Desde cualquier lugar de tu sistema.
# Construye el blog y la tienda simultáneamente.
axes innovatech-website/build_all

# Ejecuta los linters uno tras otro.
axes innovatech-website/quality_check
```

Y si solo quieres ejecutar individualmente, solo necesitas llamar a su función:

```sh
# Ejecuta el script solo para el proyecto blog.
axes innovatech-website/blog/build

axes */store/build # si ya ejecutaste el comando anterior, '*' indica que se devuelve el proyecto más recientemente utilizado del padre.
```

Has pasado de gestionar comandos individuales a orquestar flujos de trabajo completos. La complejidad de cada subproyecto está encapsulada, y el proyecto padre proporciona una API simple y potente para interactuar con el conjunto.

## 6. Flujo de Trabajo Inmersivo: Modo Sesión (`start`)

Componer y orquestar scripts es increíblemente potente. Pero a veces, solo quieres enfocarte en una única parte de tu sistema, como el blog.

Para esto, `axes` ofrece **sesiones de proyecto**.

Para entrar en el contexto del proyecto `blog`:

```sh
$ axes innovatech-website/blog start

--- Sesión de `axes` para 'innovatech-website/blog' iniciada. Escribe 'exit' para salir. ---
# El prompt de tu terminal podría cambiar para reflejar la sesión activa.
```

Ahora estás "dentro" del proyecto `blog`. `axes` ha hecho dos cosas por ti:

1. **Activación de Hook:** Ha ejecutado el script definido en `[options].at_start` de tu `axes.toml`. Esto es perfecto para activar entornos virtuales o iniciar servicios necesarios.
2. **Contexto Implícito:** Ya no necesitas especificar el contexto. `axes` sabe dónde estás.

Dentro de la sesión, tu flujo de trabajo se vuelve increíblemente simple y utiliza la misma gramática universal:

```sh
# El contexto ahora es implícito.
(axes: innovatech-website/blog) $ axes build
(axes: innovatech-website/blog) $ axes generate_footer

# ... después de una productiva sesión de trabajo ...
(axes: innovatech-website/blog) $ exit
```

Al salir, `axes` ejecuta automáticamente el hook `at_exit`, ideal para detener servicios (`docker-compose down`) y asegurar que no queden procesos huérfanos.

Las sesiones de `axes` eliminan la última barrera de fricción, permitiéndote concentrarte al 100% en tu código.

---

## ¡Has Completado el Tour! ¿Qué Sigue?

¡Felicidades! Has instalado `axes`, construido un monorepo desde cero, compartido configuración a través de la herencia, compuesto flujos de trabajo complejos y experimentado la fluidez de las sesiones de proyecto.

Ahora tienes una base sólida para comenzar a usar `axes` en tus propios proyectos.

El viaje no termina aquí. `axes` es una herramienta profunda con muchas más características diseñadas para hacer tu vida más fácil. Para convertirte en un usuario experto, te recomendamos explorar el resto de nuestra documentación:

* **[Referencia Completa de Comandos (`COMMANDS.md`)](./COMMANDS.md):** ¿Quieres saber todo lo que pueden hacer `init`, `tree`, `link` o `delete`? Esta es tu guía de referencia.
* **[Dominando `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** La guía definitiva sobre la sintaxis de `axes.toml`. Aprende sobre comandos multiplataforma, la sintaxis completa de `<params::...>` y más.
* **[Guía Técnica y de Contribución (`TECNICAL.md`)](./TECNICAL.md):** Si tienes curiosidad sobre cómo funciona `axes` internamente o quieres contribuir al proyecto, este es tu punto de partida.

## Únete a la Comunidad

`axes` se encuentra en **fase Beta** y se nutre del feedback de usuarios como tú.

* **Encontraste un Bug o Tienes una Idea:** [**Abre un Issue**](https://github.com/RetypeOS/axes/issues)
* **Quieres Contribuir con Código:** ¡Los Pull Requests son bienvenidos!

Gracias por tomarte el tiempo de aprender `axes`. ¡Esperamos ver los increíbles flujos de trabajo que construirás!
