<p align="center">
  <img src="logo.png" alt="axes Logo" width="200">
</p>

<h1 align="center">axes: El Director de Orquesta para Tu Caos de Desarrollo</h1>

<p align="center">
  <strong>Orquestación a Escala. Ergonomía por Diseño. Rendimiento por Obsesión.</strong>
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="CI/CD Status"></a>
  <a href="https://github.com/retypeos/axes/releases"><img src="https://imgshields.io/badge/version-v0.2.0--beta-blue" alt="Version"></a>
  <a href="https://deepwiki.com/RetypeOS/axes"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License"></a>
</p>

<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="./README.md">English</a> •
  <a href="./docs/es/README.md">Español</a>
</p>

---

## Tu Flujo de Trabajo Está Roto

- **Terminal 1:** `cd frontend && npm run dev`
- **Terminal 2:** `cd backend && source .venv/bin/activate && uvicorn app:main --reload`
- **Tú, 3 semanas después:** *«Espera... ¿el comando para los tests era `npm test`, `pytest`, `cargo test` o `go test ./...`?»*

Esa micro-pausa, esa carga cognitiva, es fricción. Mata tu `flow`. Herramientas como `make` o `just` te dan atajos. **`axes` te da un lenguaje universal.**

`axes` no es solo otro gestor de tareas. Es el **lenguaje de comandos** que une toda tu pila tecnológica. Te permite componer, parametrizar y estandarizar flujos de trabajo para CUALQUIER herramienta, en CUALQUIER lenguaje, a través de CUALQUIER estructura de proyecto. Tu `package.json` conoce `npm`, tu `Makefile` conoce `make`. **`axes` los conoce a todos.** Es el director de orquesta que convierte tu caótica colección de herramientas en una sinfonía.

### ¿Por Qué `axes`? Porque la Velocidad No es Suficiente

Los gestores de tareas simples son rápidos. Pero el desarrollo moderno no se trata solo de ejecutar un comando rápidamente. Se trata de gestionar la complejidad a través de docenas de ellos.

Imagina un monorepo:

**EL CAOS (ANTES de `axes`):**

```sh
# Para levantar todo...
(terminal 1) $ cd frontend && npm run dev
(terminal 2) $ cd backend && source .venv/bin/activate && flask run
(terminal 3) $ cd docs && hugo server
```

**LA ORQUESTA (CON `axes`):**

```toml
# en ./.axes/axes.toml
[scripts]
# El '>' indica ejecución en paralelo.
dev = [
    "> axes frontend dev", # Llama al script dev del proyecto frontend
    "> axes backend dev",  # Llama al script dev del proyecto backend
    "> axes docs dev"      # Y al script dev del proyecto docs
]
```

A partir de ahora, cualquier miembro de tu equipo, en cualquier máquina, ejecuta todo el entorno con **un solo comando universal**:

```sh
axes dev
```

Acabas de convertir el conocimiento tribal en infraestructura versionada. El onboarding de un nuevo desarrollador pasó de horas a segundos.

---

### La Filosofía `axes`: Más que un Gestor de Tareas

`axes` está construido sobre una base que los gestores de tareas simples ignoran.

- **Orquestación, no solo Ejecución:** `axes` entiende que los proyectos tienen relaciones. Organízalos en árboles (`app/api`, `app/web`). Los hijos heredan y sobrescriben variables y scripts. Define una vez, usa en todas partes. Esto es DRY (No te Repitas) a un nivel completamente nuevo.
- **Ergonomía, no solo Atajos:** Tus scripts se convierten en aplicaciones de línea de comandos de primera clase.

    ```toml
    # Scripts como Funciones: Parametriza, valida y establece valores por defecto.
    [scripts]
    deploy = "terraform apply -var 'env=<axes::params::0(default='staging')>'"
    ```

    No más scripts `bash` frágiles para parsear argumentos.
- **Rendimiento, sin Compromiso:** Escrito en Rust, `axes` está diseñado para la velocidad. Un motor de caché de estilo JIT (Just-In-Time) compila tus flujos de trabajo a un formato binario. La primera ejecución paga el precio de la orquestación; **cada ejecución subsecuente es casi instantánea.** Obtienes la potencia de un sistema complejo a la velocidad de uno simple.

---

### Instalación (30 Segundos para un Mejor Flujo de Trabajo)

`axes` es un único binario sin dependencias.

1. Ve a la [**página de Releases de `axes` en GitHub**](https://github.com/RetypeOS/axes/releases).
2. Descarga el archivo para tu sistema operativo.
3. Descomprímelo y mueve el ejecutable `axes` a un directorio en el `PATH` de tu sistema.
4. Abre una **nueva terminal** y verifica con `axes --version`.

---

### `axes` en Acción: Un Vistazo al Poder

Mientras tú buscas en un `README` antiguo, otros ya están orquestando.

#### 1. Comandos Universales y Conscientes del Contexto

Ejecuta un script en el directorio actual. La sintaxis es simple y predecible.

```sh
# Ejecuta el script 'build' definido en el axes.toml más cercano
axes build --release
```

#### 2. Flujos de Trabajo Multiplataforma sin Esfuerzo

Define un comando una vez. Funciona para todo tu equipo, en cualquier SO.

```toml
[scripts.browse]
desc = "Abre la documentación local en el navegador."
windows = "start http://localhost:8080"
macos   = "open http://localhost:8080"
linux   = "xdg-open http://localhost:8080"
```

#### 3. Composición Dinámica y en Tiempo Real

Ejecuta comandos y usa su salida sobre la marcha.

```toml
[scripts]
# Etiqueta una imagen Docker con el hash git corto actual
tag_release = "docker tag my-app:latest my-app:<axes::run('git rev-parse --short HEAD')>"
```

#### 4. Sesiones de Enfoque Inmersivo

Sumérgete en un sub-proyecto. `axes` configura y desmonta tu entorno por ti.

```toml
# en mi-app/api/.axes/axes.toml
[options]
at_start = "source .venv/bin/activate" # Se ejecuta al entrar
at_exit  = "docker-compose down"       # Se ejecuta al salir
```

```sh
$ axes my-app/api # Inicia una sesión. `at_start` se ejecuta automáticamente.

(axes: my-app/api) $ axes test  # No necesitas repetir el contexto.
(axes: my-app/api) $ exit       # `at_exit` se ejecuta automáticamente.
```

**Tu entorno de desarrollo, bajo demanda.**

---

### ¿Listo para Dirigir Tu Propia Orquesta?

La fricción que sientes cada día no es un requisito. Es un problema con una solución. `axes` es esa solución.

- ➡️ **[Guía de Inicio Rápido (`GETTING_STARTED.md`)](./GETTING_STARTED.md):** Construye tu primer monorepo orquestado en 15 minutos.
- 📖 **[Dominando el `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** La referencia definitiva de cada característica.
- ⌨️ **[Referencia de Comandos (`COMMAND.md`)](./COMMAND.md):** Una guía completa de cada comando de la CLI.

### Únete a la Revolución del Flujo de Trabajo

`axes` es más que una herramienta; es un movimiento para restaurar el control y la coherencia en el desarrollo. Tu voz es crucial.

- **Encuentra un Bug o tienes una Idea Genial:** [**Abre un Issue**](https://github.com/RetypeOS/axes/issues)
- **Quieres Contribuir con Código:** ¡Los Pull Requests son siempre bienvenidos!

**Instala `axes` hoy. Deja de buscar comandos. Olvídate de los pequeños problemas. Céntrate en lo que realmente importa: **darle vida a tu software**, y deja que `axes` se preocupe del cómo.**
