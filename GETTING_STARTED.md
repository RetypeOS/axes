# Guía de Inicio: Tu Primer Monorepo Orquestado con `axes`

¡Bienvenido a `axes`! Esta guía te llevará desde cero hasta tener un monorepo completamente funcional y orquestado. En los próximos 15-20 minutos, aprenderás a:

* ✅ Instalar `axes` en tu sistema.
* ✅ Crear tu primer proyecto y sub-proyectos.
* ✅ Definir y ejecutar scripts.
* ✅ Aprovechar la herencia de variables entre proyectos.
* ✅ Orquestar un flujo de trabajo complejo que involucra múltiples proyectos.
* ✅ Usar sesiones de proyecto para un flujo de trabajo enfocado.

Al final de este tutorial, entenderás el poder fundamental de `axes` y estarás listo para aplicarlo a tus propios proyectos.

---

## 1. Instalación

`axes` es un único archivo binario sin dependencias externas, lo que hace que su instalación sea muy sencilla.

### Opción A: Descargar el Binario Pre-compilado (Recomendado)

Esta es la forma más rápida de empezar.

1. **Ve a la página de Releases:** Abre la [página oficial de Releases de `axes` en GitHub](https://github.com/RetypeOS/axes/releases).
2. **Descarga el archivo correcto:** Busca la última versión y descarga el archivo `.zip` o `.tar.gz` que corresponda a tu sistema operativo (Windows, macOS, o Linux).
3. **Descomprime el archivo:** Dentro encontrarás un único ejecutable: `axes.exe` (en Windows) o `axes` (en macOS/Linux).
4. **Mueve el ejecutable a tu `PATH`:** Este es el paso más importante. Para poder llamar a `axes` desde cualquier lugar en tu terminal, debes mover este archivo a un directorio que esté en la variable de entorno `PATH` de tu sistema.

    * **Windows:**
        1. Crea una carpeta, por ejemplo, `C:\Program Files\axes`.
        2. Mueve `axes.exe` a esa carpeta.
        3. Busca "Editar las variables de entorno del sistema" en el menú de inicio, abre el editor de `PATH` y añade la ruta `C:\Program Files\axes` a la lista.
    * **macOS / Linux:**
        Un directorio común y recomendado es `/usr/local/bin`. Puedes mover el archivo con este comando en tu terminal (puede que necesites `sudo`):

        ```sh
        sudo mv ./axes /usr/local/bin/axes
        ```

5. **Verifica la instalación:** Abre una **nueva** ventana de terminal (esto es importante para que se carguen los cambios en el `PATH`) y ejecuta:

    ```sh
    axes --version
    ```

    Si ves un número de versión, ¡la instalación ha sido un éxito!

### Opción B: Compilar desde el Código Fuente

Si tienes el [toolchain de Rust](https://www.rust-lang.org/tools/install) instalado, puedes compilar `axes` tú mismo.

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

Con `axes` instalado, estás listo para crear tu primer proyecto. ¡Vamos allá!

## 2. Nuestro Escenario y la Navegación de Contextos

Para este tutorial, construiremos la estructura de un sitio web corporativo ficticio llamado "Innovatech". Este sitio tendrá dos componentes principales: un **blog** y una **tienda online**.

Antes de empezar, es crucial entender cómo `axes` se refiere a los proyectos. Al igual que navegas por un sistema de archivos con `cd`, `axes` navega por su árbol de proyectos lógicos usando **contextos**. Aquí está la tabla de referencia rápida:

| Contexto | Descripción                                                                | Ejemplo (desde `.../innovatech-website/blog`)               |
| :------- | :------------------------------------------------------------------------- | :---------------------------------------------------------- |
| `nombre` | Un hijo directo del proyecto raíz (por defecto llamado `global`).          | `axes innovatech-website info`                              |
| `/`      | El separador de niveles en la jerarquía.                                   | `axes innovatech-website/blog info`                         |
| `.`      | El proyecto del directorio actual, o el primer ancestro encontrado.        | `axes . info` (resuelve a `innovatech-website/blog`)        |
| `_`      | El proyecto cuyo directorio raíz es *exactamente* el directorio actual.    | `axes _ info` (resuelve a `innovatech-website/blog`)        |
| `..`     | El padre del proyecto de contexto actual o el primer ancestro encontrado.  | `axes . .. info` (resuelve a `innovatech-website`)          |
| `alias!` | Un atajo personalizado que creas.                                          | `axes blog! info` (si `blog!` apunta a nuestro proyecto)    |

A lo largo de este tutorial, usaremos estos contextos para que veas lo fluidos y potentes que son.

### Creando el Proyecto Contenedor

Primero, crea un directorio para todo el monorepo y, dentro de él, inicializa tu proyecto `axes` raíz.

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
check_copyright = "echo \"© $(date +%Y) <axes::vars::company_name>. Todos los derechos reservados.\""
```

Hemos definido una variable y un script que actuarán como configuración compartida para todo nuestro monorepo.

---

## 3. El Primer Sub-Proyecto: El Blog

Ahora, vamos a crear el blog como un **hijo** de `innovatech-website`.

```sh
# Dentro de innovatech-website/, crea y entra al directorio del blog
mkdir blog && cd blog

# Inicializa `axes`, usando `..` para referirse al padre (`innovatech-website`)
axes init --parent ..
```

En el asistente, `axes` interpretará `..` como el proyecto en el directorio padre o superior y te lo sugerirá. ¡Ya estás usando la navegación por contexto!

Para visualizar la nueva estructura, sube un nivel y usa `.`:

```sh
# Desde el directorio innovatech-website/
cd ..
axes innovatech-website tree

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

Ahora, abre el `axes.toml` dentro de `blog/` y define un script que use la configuración heredada:

```toml
# ./innovatech-website/blog/.axes/axes.toml
version = "0.1.0"
description = "El blog de Innovatech."

[scripts]
build = "hugo --minify"
# Este script COMPONE el script 'check_copyright' heredado del padre.
generate_footer = [
    "echo '--- Generando Footer del Blog ---'",
    "<axes::scripts::check_copyright>",
    "echo 'Construido con <axes::name>'"
]
```

Ejecútalo usando el contexto `.`, ya que estamos dentro del directorio `blog`:

```sh
# Estando en el directorio blog/
axes . generate_footer
```

La salida será:

```text
> --- Generando Footer del Blog ---
> © 2024 Innovatech Inc.. Todos los derechos reservados.
> Construido con innovatech-website/blog
```

Has compartido configuración y lógica de forma limpia y navegado por tu proyecto de forma intuitiva. A continuación, añadiremos más complejidad con nuestra tienda online.

## 4. El Segundo Sub-Proyecto: La Tienda Online

Nuestra tienda online será el tercer proyecto en nuestro árbol. El proceso es idéntico al del blog.

```sh
# Desde el directorio raíz (innovatech-website/)
mkdir tienda && cd tienda

# Inicializa, especificando de nuevo el padre con `..`
axes init --parent ..
```

Tras el asistente, tu árbol de proyectos (`axes .. tree`) se verá así:

```text
innovatech-website
├─ blog
└─ tienda
```

Ahora, vamos a darle a la tienda un script más avanzado. A menudo, queremos ejecutar pruebas solo para una parte específica de nuestra aplicación. `axes` lo hace fácil definiendo scripts que actúan como "funciones" que aceptan parámetros.

Edita el nuevo `axes.toml` en `tienda/`:

```toml
# ./innovatech-website/tienda/.axes/axes.toml
version = "1.0.0"
description = "La tienda online de Innovatech."

[vars]
# Podemos sobreescribir o definir nuevas variables.
payment_gateway_api_key = "pk_test_12345"

[scripts]
# Este script de prueba acepta un parámetro posicional.
# `<axes::params::0>` será reemplazado por el primer argumento
# que le pasemos al script desde la línea de comandos.
test_module = "pytest tests/test_<axes::params::0>.py"
```

Ahora, desde la raíz del monorepo, puedes ejecutar los tests para un módulo específico de la tienda:

```sh
# Desde el directorio innovatech-website/
axes tienda test_module payments  # --> se ejecutará `pytest tests/test_payments.py`
axes tienda test_module products  # --> se ejecutará `pytest tests/test_products.py`
```

Has creado un atajo reutilizable y parametrizable, eliminando la necesidad de recordar o escribir rutas de prueba largas y complejas.

> **Profundiza:** El sistema de parámetros de `axes` es extremadamente potente, permitiendo flags, valores por defecto y más. Para dominarlo, consulta nuestra guía completa: **[Dominando el `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md)**.

---

## 5. La Orquestación Maestra

Hemos creado proyectos individuales, cada uno con sus propios scripts. Ahora, vamos a unirlos. El verdadero poder de `axes` reside en su capacidad para actuar como el director de orquesta de todo tu ecosistema.

Volvamos al `axes.toml` del proyecto padre, `innovatech-website`, para crear flujos de trabajo que controlen a los hijos.

```toml
# ./innovatech-website/.axes/axes.toml

version = "1.0.0"
description = "El monorepo para el sitio web de Innovatech."

[vars]
company_name = "Innovatech Inc."

[scripts]
check_copyright = "echo \"© $(date +%Y) <axes::vars::company_name>. Todos los derechos reservados.\""

# ¡NUEVO! Un script que llama a los scripts de sus hijos.
# El prefijo `>` indica que el comando debe ejecutarse en PARALELO.
build_all = [
    "echo '🚀 Construyendo todo el sitio web...'",
    "> axes blog build",
    "> axes tienda build" # Asumimos que `tienda` también tiene un script `build`.
]

# Un script de calidad que se ejecuta en secuencia.
quality_check = [
    "echo ' linting...'",
    "axes blog lint",  # Asumimos scripts `lint` en los hijos.
    "axes tienda lint",
    "echo '✅ Calidad del código verificada!'"
]
```

Con esta configuración, has creado puntos de entrada únicos para operaciones complejas en todo el monorepo:

```sh
# Desde cualquier lugar de tu sistema.
# Construye el blog y la tienda simultáneamente.
axes innovatech-website build_all

# Ejecuta los linters uno tras otro.
axes innovatech-website quality_check
```

Y si solo quieres ejecutar individualmente solo debes llamar a su función:

```sh
# Ejecutas el script unicamente del proyecto blog.
axes innovatech-website/blog build

axes */tienda build # si ya ejecutaste en anterior comando, '*' indica que del proyecto padre devuelve el proyecto usado más reciente.

Has pasado de gestionar comandos individuales a orquestar flujos de trabajo completos. La complejidad de cada sub-proyecto está encapsulada, y el proyecto padre proporciona una API simple y potente para interactuar con el todo.

## 6. Flujo de Trabajo Inmersivo: El Modo Sesión (`start`)

Componer y orquestar scripts es increíblemente poderoso. Pero a veces, solo quieres concentrarte en una única parte de tu sistema, como el blog. Escribir `axes innovatech-website/blog` para cada comando puede ser repetitivo.

Para esto, `axes` ofrece **sesiones de proyecto**.

Imagina que vas a pasar la próxima hora trabajando solo en el blog. Desde cualquier lugar, simplemente "entra" en su contexto:

```sh
# `start` es la acción por defecto si solo proporcionas un contexto.
# Este comando es un atajo para `axes innovatech-website/blog start`
$ axes innovatech-website/blog

--- `axes` session for 'innovatech-website/blog' started. Type 'exit' to leave. ---
# Tu prompt de la terminal podría cambiar para reflejar la sesión activa.
```

Ahora estás "dentro" del proyecto `blog`. `axes` ha hecho varias cosas por ti en segundo plano:

1. **Activación de Hooks:** Ha ejecutado el script definido en `[options].at_start` de tu `axes.toml`. Esto es perfecto para activar entornos virtuales (`source .venv/bin/activate`), exportar variables de entorno (`export FLASK_ENV=development`), o iniciar servicios necesarios.
2. **Contexto Implícito:** Ya no necesitas especificar el contexto. `axes` sabe dónde estás.

Dentro de la sesión, tu flujo de trabajo se vuelve increíblemente simple:

```sh
# No más `axes innovatech-website/blog ...`
(axes: innovatech-website/blog) $ axes build
(axes: innovate-website/blog) $ axes generate_footer

# ... después de un productivo rato de trabajo ...
(axes: innovatech-website/blog) $ exit
```

Al salir, `axes` ejecuta automáticamente el hook `at_exit`, ideal para detener servicios (`docker-compose down`) y asegurar que no queden procesos huérfanos.

Las sesiones de `axes` eliminan la última barrera de fricción, permitiéndote concentrarte al 100% en tu código.

---

## ¡Has Completado el Tour! ¿Qué Sigue?

¡Felicidades! Has instalado `axes`, construido un monorepo desde cero, compartido configuración mediante herencia, compuesto flujos de trabajo complejos y experimentado la fluidez de las sesiones de proyecto.

Ahora tienes una base sólida para empezar a usar `axes` en tus propios proyectos.

El viaje no termina aquí. `axes` es una herramienta profunda con muchas más características diseñadas para hacer tu vida más fácil. Para convertirte en un usuario experto, te recomendamos explorar el resto de nuestra documentación:

* **[Referencia Completa de Comandos (`COMMANDS.md`)](./COMMANDS.md):** ¿Quieres saber todo lo que `init`, `tree`, `link` o `delete` pueden hacer? Esta es tu guía de referencia.
* **[Dominando el `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** La guía definitiva sobre la sintaxis del `axes.toml`. Aprende sobre comandos multiplataforma, la sintaxis completa de `<axes::params::...>`, y más.
* **[Guía Técnica y de Contribución (`TECNICAL.md`)](./TECNICAL.md):** Si sientes curiosidad por cómo funciona `axes` por dentro o quieres contribuir al proyecto, este es tu punto de partida.

## Únete a la Comunidad

`axes` está en **fase Beta** y prospera gracias al feedback de usuarios como tú.

* **Encuentra un Bug o tienes una Idea:** [**Abre un Issue**](https://github.com/RetypeOS/axes/issues)
* **Quieres Contribuir con Código:** ¡Los Pull Requests son bienvenidos!

Gracias por tomarte el tiempo de aprender `axes`. ¡Estamos ansiosos por ver los increíbles flujos de trabajo que construirás!
