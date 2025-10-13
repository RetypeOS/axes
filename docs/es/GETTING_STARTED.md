<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../GETTING_STARTED.md">English</a> •
  <a href="./GETTING_STARTED.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../GETTING_STARTED.md), que es la fuente canónica de la documentación.

# Guía de Inicio Rápido: Tu Primer Monorepo Orquestado con `axes`

¡Bienvenido a `axes`! Esta guía te llevará de cero a un monorepo totalmente funcional y orquestado. En los próximos 15-20 minutos, aprenderás a:

* ✅ Instalar `axes` en tu sistema.
* ✅ Crear tu primer proyecto y sub-proyectos.
* ✅ Definir y ejecutar *scripts* usando la gramática universal nueva.
* ✅ Aprovechar la herencia de variables entre proyectos.
* ✅ Orquestar un flujo de trabajo complejo que involucra múltiples proyectos.
* ✅ Usar sesiones de proyecto para un flujo de trabajo enfocado.

Al final de este tutorial, comprenderás el poder fundamental de `axes` y estarás listo para aplicarlo a tus propios proyectos.

---

## 1. Instalación

`axes` es un único binario sin dependencias externas, lo que hace que su instalación sea muy sencilla.

### Opción A: Descargar el Binario Precompilado (Recomendado)

Esta es la forma más rápida de empezar.

1. **Ve a la Página de *Releases***: Abre la [página oficial de *Releases* de `axes` en GitHub](https://github.com/RetypeOS/axes/releases).
2. **Descarga el archivo correcto**: Busca la última versión y descarga el archivo `.zip` o `.tar.gz` que corresponda a tu sistema operativo (Windows, macOS o Linux).
3. **Descomprime el archivo**: Dentro, encontrarás un único ejecutable: `axes.exe` (en Windows) o `axes` (en macOS/Linux).
4. **Mueve el ejecutable a tu `PATH`**: Este es el paso más importante. Para poder llamar a `axes` desde cualquier lugar de tu terminal, debes mover este archivo a un directorio que esté en la variable de entorno `PATH` de tu sistema.

    * **Windows:**
        1. Crea una carpeta, por ejemplo, `C:\Program Files\axes`.
        2. Mueve `axes.exe` a esa carpeta.
        3. Busca "Editar las variables de entorno del sistema" en el menú de inicio, abre el editor de `PATH` y añade la ruta `C:\Program Files\axes` a la lista.
    * **macOS / Linux:**
        Un directorio común y recomendado es `/usr/local/bin`. Puedes mover el archivo con este comando en tu terminal (podrías necesitar `sudo`):

        ```sh
        sudo mv ./axes /usr/local/bin/axes
        ```

5. **Verifica la instalación**: Abre una **nueva** ventana de terminal (esto es importante para que se carguen los cambios de `PATH`) y ejecuta:

    ```sh
    axes --version
    ```

    Si ves un número de versión, ¡la instalación fue un éxito!

### Opción B: Compilar desde el Código Fuente

Si tienes instalado el [toolchain de Rust](https://www.rust-lang.org/tools/install), puedes compilar `axes` tú mismo.

```sh
# 1. Clona el repositorio
git clone https://github.com/RetypeOS/axes.git

# 2. Navega al directorio
cd axes

# 3. Compila en modo release (optimizado)
cargo build --release
```

El ejecutable final se ubicará en `./target/release/axes`. Puedes mover este archivo a tu `PATH` como se describe en la Opción A.

---

Con `axes` instalado, estás listo para crear tu primer proyecto. ¡Vamos!

## 2. Nuestro Escenario y Navegación de Contexto

Para este tutorial, construiremos la estructura de un sitio web corporativo ficticio llamado "Innovatech." Este sitio tendrá dos componentes principales: un **blog** y una **tienda online**.

Antes de empezar, es crucial entender cómo `axes` se refiere a los proyectos. Al igual que navegas por un sistema de archivos con `cd`, `axes` navega por su árbol lógico de proyectos usando **contextos**. Estos se utilizan para indicar a comandos como `info`, `tree` o `start` sobre qué proyecto operar.

| Contexto | Descripción                                                                 | Ejemplo (desde `.../innovatech-website/blog`) |
| :------- | :-------------------------------------------------------------------------- | :------------------------------------------ |
| `name`   | Un hijo directo del proyecto raíz (el nombre por defecto es `global`).       | `axes innovatech-website info`              |
| `/`      | El separador de nivel en la jerarquía.                                      | `axes innovatech-website/blog info`         |
| `.`      | El proyecto en el directorio de trabajo actual.                             | `axes . info` (se resuelve a `innovatech-website/blog`)     |
| `_`      | El proyecto cuya raíz es *exactamente* el directorio actual.                | `axes _ info` (se resuelve a `innovatech-website/blog`)     |
| `..`     | El padre del proyecto de contexto actual o busca en la ruta superior.       | `axes .. info` (se resuelve a `innovatech-website`)       |
| `**`     | (Doble asterisco) Se resuelve al último proyecto que usaste en **todo el sistema.** Útil para volver rápidamente. | `axes ** start`       |
| `*`      | (Asterisco simple) Se resuelve al último hijo que usaste **del proyecto padre actual**. | `axes mi-super-app/* start`           |
| `alias!` | Un atajo personalizado que creas.                                           | `axes blog! info` (si `blog!` apunta a nuestro proyecto) |

A lo largo de este tutorial, usaremos estos contextos para que puedas ver lo fluidos y potentes que son.

### Creación del Proyecto Contenedor

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

Hemos definido una variable y un *script* que actuarán como configuración compartida para todo nuestro monorepo.

---

## 3. El Primer Sub-Proyecto: El Blog

Ahora, creemos el blog como un **hijo** de `innovatech-website`.

```sh
# Dentro de innovatech-website/, crea y entra al directorio del blog
mkdir blog && cd blog

# Inicializa `axes`, usando `..` para referirte al padre (`innovatech-website`)
axes init --parent ..
```

En el asistente, `axes` interpretará `..` como el proyecto en el directorio padre o superior y te lo sugerirá. ¡Ya estás usando la navegación de contexto!

Para visualizar la nueva estructura, vuelve al directorio padre y ejecuta `tree`:

```sh
# Desde el directorio innovatech-website/
cd ..
axes tree # Implícito por '.'

# O, de forma más inteligente, desde dentro de `blog/`:
# "Muéstrame el árbol de mi padre"
axes .. tree
```

Ambos mostrarán:

```text
innovatech-website
└─ blog
```

### Demostrando la Herencia

Ahora abre el `axes.toml` dentro de `blog/` y define un *script* que use la configuración heredada:

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

Para ejecutar un *script* en tu proyecto actual, simplemente usa su nombre.

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

Has compartido configuración y lógica de forma limpia y has navegado por tu proyecto de forma intuitiva. A continuación, añadiremos más complejidad con nuestra tienda online.

## 4. El Segundo Sub-Proyecto: La Tienda Online

Nuestra tienda online será el tercer proyecto en nuestro árbol. El proceso es idéntico al del blog.

```sh
# Desde el directorio raíz (innovatech-website/)
mkdir store && cd store

# Inicializa, especificando de nuevo el padre con `..`
axes init --name store --parent ..
```

Después del asistente, tu árbol de proyectos (`axes innovatech-website tree`) se verá así:

```text
innovatech-website
├─ blog
└─ store
```

Ahora, demos al *store* un *script* más avanzado. Edita el nuevo `axes.toml` en `store/`:

```toml
# ./innovatech-website/store/.axes/axes.toml
version = "1.0.0"
description = "La tienda online de Innovatech."

[scripts]
# Este script de test acepta un parámetro posicional.
# `<params::0>` será reemplazado por el primer argumento que pasemos
# al script desde la línea de comandos.
test_module = "pytest tests/test_<params::0>.py"
```

Para ejecutar un *script* en un **proyecto diferente**, usarás el comando `run` explícitamente o ./nombre_del_script. Esto elimina ambigüedades y hace tu intención clara.

```sh
# Desde el directorio innovatech-website/
axes store run test_module payments  # --> ejecutará `pytest tests/test_payments.py` (Modo explícito con `run`)
axes store/test_module products  # --> ejecutará `pytest tests/test_products.py` (Modo implícito con <ctx>/script_name [args...])
```

Has creado un atajo reutilizable y parametrizable, eliminando la necesidad de recordar o escribir rutas de prueba largas y complejas.

> **Profundiza:** El sistema de parámetros de `axes` es extremadamente potente, permitiendo *flags*, valores por defecto y más. Para dominarlo, consulta nuestra guía completa: **[Dominando `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.

### Una Mirada Profunda: La Gramática Universal de Comandos

El comando `axes store/test_module payments` es un claro ejemplo de la característica más potente y ergonómica de `axes`: su **gramática universal de comandos**.

A diferencia de los *task runners* tradicionales que a menudo requieren un subcomando específico como `run` para ejecutar un *script* (`task_runner run <script> -- <args>`), `axes` trata la combinación de `<contexto>/<script_name>` como un único comando directamente ejecutable.

Esto crea un **"espacio de comandos virtual"** donde cada *script* de tu monorepo se vuelve accesible como si fuera un binario nativo en tu `PATH`.

#### Cómo Funciona

El *dispatcher* de `axes` está diseñado para analizar inteligentemente esta gramática. Cuando recibe un comando, divide el primer argumento en la *última* `/` que encuentra:

```sh
axes store/test_page payments
#    └─┬─┘ └───┬─────┘ └───┬───···>
#      │       │           └─ Parámetros (pasados al script)
#      │       └─ Nombre del Script
#      └─ Contexto
```

* Todo lo que está antes de la última `/` (`store`) se trata como el **contexto**.
* Todo lo que está después (`test_module`) es el **nombre del *script*** a ejecutar dentro de ese contexto.
* Cualquier argumento posterior (`payments`) se pasa como **parámetros** al *script*.

Esta elección de diseño es intencional. Hace que los *scripts* de tu proyecto se sientan como comandos nativos y de primera clase, reduciendo la carga cognitiva y haciendo que tu flujo de trabajo se sienta fluido e integrado con tu *shell*. Esto convierte a todo tu monorepo en una única aplicación de línea de comandos cohesiva.

---

## 4. Orquestación Maestra

Hemos creado proyectos individuales, cada uno con sus propios *scripts*. Ahora, juntémoslos. El verdadero poder de `axes` reside en su capacidad para actuar como el director de orquesta de todo tu ecosistema.

Volvamos al `axes.toml` del proyecto padre, `innovatech-website`, para crear flujos de trabajo que controlen a los hijos.

```toml
# ./innovatech-website/.axes/axes.toml

version = "1.0.0"
description = "El monorepo para el sitio web de Innovatech."

[vars]
company_name = "Innovatech Inc."

[scripts]
check_copyright = "echo \"© $(date +%Y) <vars::company_name>. Todos los derechos reservados.\""

# Un script que llama a los scripts de sus hijos.
build_all = [
    "# 🚀 Construyendo todo el sitio web...",
    # El prefijo `>` indica que el comando debe ejecutarse en PARALELO.
    # Usamos el comando explícito `run` para mayor claridad y robustez.
    "@> axes blog/build",
    "@> axes store/build" # Asumiendo que `store` también tiene un script `build`.
]

# Un script de calidad que se ejecuta en secuencia.
quality_check = [
    "# Linting...",
    "@ axes blog/lint",  # Asumiendo que existen scripts `lint` en los hijos.
    "@ axes store/lint",
    "# ✅ Calidad de código verificada!"
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
# Ejecuta el script solo para el proyecto del blog.
axes innovatech-website/blog/build

axes */store/build # si ya ejecutaste el comando anterior, '*' indica que se devuelve el proyecto más recientemente usado del padre.
```

Has pasado de gestionar comandos individuales a orquestar flujos de trabajo completos. La complejidad de cada sub-proyecto está encapsulada, y el proyecto padre proporciona una API simple y potente para interactuar con el todo.

## 5. Flujo de Trabajo Inmersivo: Modo Sesión (`start`)

Componer y orquestar *scripts* es increíblemente poderoso. Pero a veces, solo quieres centrarte en una parte de tu sistema, como el blog.

Para esto, `axes` ofrece **sesiones de proyecto**.

Para entrar al contexto del proyecto `blog`:

```sh
$ axes innovatech-website/blog start

--- Sesión de `axes` para 'innovatech-website/blog' iniciada. Escribe 'exit' para salir. ---
# Tu prompt de terminal podría cambiar para reflejar la sesión activa.
```

Estás ahora "dentro" del proyecto `blog`. `axes` ha hecho dos cosas por ti:

1. **Activación de Hook:** Ha ejecutado el *script* definido en `[options].at_start` de tu `axes.toml`. Esto es perfecto para activar entornos virtuales o iniciar servicios necesarios.
2. **Contexto Implícito:** Ya no necesitas especificar el contexto. `axes` sabe dónde estás.

Dentro de la sesión, tu flujo de trabajo se vuelve increíblemente simple y usa la misma gramática universal:

```sh
# El contexto ahora es implícito.
(axes: innovatech-website/blog) $ axes build
(axes: innovatech-website/blog) $ axes generate_footer

# ... después de una sesión de trabajo productiva ...
(axes: innovatech-website/blog) $ exit
```

Al salir, `axes` ejecuta automáticamente el *hook* `at_exit`, ideal para detener servicios (`docker-compose down`) y asegurar que no queden procesos huérfanos.

Las sesiones de `axes` eliminan la última barrera de fricción, permitiéndote centrarte al 100% en tu código.

---

## ¡Has Completado el Recorrido! ¿Qué Sigue?

¡Felicidades! Has instalado `axes`, construido un monorepo desde cero, compartido configuración mediante herencia, compuesto flujos de trabajo complejos y experimentado la fluidez de las sesiones de proyecto.

Ahora tienes una base sólida para comenzar a usar `axes` en tus propios proyectos.

El viaje no termina aquí. `axes` es una herramienta profunda con muchas más características diseñadas para facilitarte la vida. Para convertirte en un usuario experto, te recomendamos explorar el resto de nuestra documentación:

* **[Referencia Completa de Comandos (`COMMANDS.md`)](./COMMANDS.md)**: ¿Quieres saber todo lo que `init`, `tree`, `link` o `delete` pueden hacer? Esta es tu guía de referencia.
* **[Dominando `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**: La guía definitiva de la sintaxis de `axes.toml`. Aprende sobre comandos multiplataforma, la sintaxis completa de `<params::...>` y más.
* **[Guía Técnica y de Contribución (`TECNICAL.md`)](./TECNICAL.md)**: Si tienes curiosidad sobre cómo funciona `axes` internamente o quieres contribuir al proyecto, este es tu punto de partida.

## Únete a la Comunidad

`axes` está en **Fase Beta** y prospera con los comentarios de usuarios como tú.

* **¿Encontraste un *Bug* o tienes una Idea?:** [**Abre un *Issue***](https://github.com/RetypeOS/axes/issues)
* **¿Quieres Contribuir con Código?:** ¡Los *Pull Requests* son bienvenidos!

Gracias por tomarte el tiempo de aprender `axes`. ¡Esperamos ver los increíbles flujos de trabajo que construirás!
