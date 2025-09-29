<p align="center">
  <img src="../../logo.png" alt="axes Logo" width="200">
</p>
<h1 align="center">axes: El Director de Orquesta para Tu Caos de Desarrollo</h1>

<p align="center">
  <strong>Cualquier Proyecto. Cualquier Lenguaje. Un Solo Lenguaje de Comandos.</strong>
</p>

<p align="center">
  <a href="#"><img src="https://img.shields.io/badge/build-passing-brightgreen" alt="CI/CD Status"></a>
  <a href="#"><img src="https://img.shields.io/badge/version-v0.2.0--beta-blue" alt="Version"></a>
  <a href="https://deepwiki.com/RetypeOS/axes"><img src="https://deepwiki.com/badge.svg" alt="Ask DeepWiki"></a>
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-lightgrey" alt="License"></a>
</p>

<p align="center">
  <strong>Read this in other languages:</strong><br>
  <a href="../../README.md">English</a> •
  <a href="README.md">Español</a>
</p>

> **Nota:** Esta traducción es mantenida por la comunidad y podría no estar completamente sincronizada con la [versión en inglés](../../README.md), que es la fuente canónica de la documentación.

---

## ¿Tu flujo de trabajo se ve así?

- **Terminal 1:** `cd frontend && npm run dev`
- **Terminal 2:** `cd backend && source .venv/bin/activate && uvicorn app:main --reload`
- **Tú, 3 semanas después:** *«Espera... ¿el comando para los tests era `npm test`, `pytest`, `cargo test` o `go test ./...`?»*

Esa micro-pausa, esa carga cognitiva al cambiar de proyecto, es una fricción que se acumula. Te roba el `flow`. Te roba la productividad. **Otras herramientas te dan atajos. `axes` te da un lenguaje.**

`axes` no es otro gestor de paquetes ni una alternativa a `Docker` o `make`. Es el **lenguaje de comandos** que los une a todos. `axes` te permite componer, parametrizar y estandarizar flujos de trabajo que involucran CUALQUIER herramienta de tu stack tecnológico. Tu `package.json` sabe cómo ejecutar `npm`, tu `Makefile` sabe cómo ejecutar `make`, y tu `docker-compose.yml` sabe cómo ejecutar `Docker`. Pero, ¿quién sabe cómo ejecutarlos a **todos juntos**? **`axes` es esa inteligencia faltante.**. Es el director de orquesta que les dice qué hacer, usando comandos simples, coherentes y poderosos que **TÚ** defines y que viajan con tu repositorio, permitiendo que nuevos usuarios puedan unirse de forma absolutamente sencilla y estándar.

### ¿Por qué `axes`? ¿Por qué ahora?

El mundo del desarrollo es un caos de herramientas. Cada proyecto tiene su propio dialecto de comandos. `axes` introduce un `esperanto` para tu terminal.

Imagina un `monorepo` con un frontend, un backend y un servicio de documentación:

**EL CAOS (ANTES):**

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
    "> axes frontend dev", # Llama al script `dev` del hijo `frontend`
    "> axes backend dev",  # Llama al script `dev` del hijo `backend`
    "> axes docs dev"      # Llama al script `dev` del hijo `docs`
]
```

A partir de ahora, cualquier miembro del equipo, en cualquier máquina, levanta todo el entorno con **un solo comando universal**:

```sh
axes . dev
```

Has convertido el conocimiento tribal en infraestructura versionada. El onboarding de nuevos desarrolladores acaba de pasar de horas a segundos.

---

### La Filosofía `axes`

- **Abstracción, no Reemplazo:** `axes` no es un nuevo gestor de paquetes. Usa las herramientas que ya amas.
- **Convención sobre Configuración (Tu Convención):** Define tus propios comandos estándar (`dev`, `test`, `lint`, `deploy`) y úsalos en todos tus proyectos, sin importar la tecnología subyacente.
- **Jerarquía y Herencia (DRY al Máximo):** Organiza proyectos en árboles (`mi-app/api`, `mi-app/frontend`). Los hijos heredan y pueden sobrescribir variables y scripts de sus padres. Define una vez, usa en todas partes.
- **Agnóstico al SO (Verdadera Portabilidad):** Define flujos de trabajo que funcionan sin problemas en Windows, macOS y Linux. `axes` se encarga de ejecutar el comando correcto para cada plataforma.
- **Infraestructura como Código:** Tu `axes.toml` vive en Git. Tus flujos de trabajo evolucionan con tu código.

---

### Instalación (30 segundos para empezar)

`axes` es un único binario sin dependencias.

1. Ve a la página de [**Releases de `axes` en GitHub**](https://github.com/RetypeOS/axes/releases).
2. Descarga el archivo para tu sistema operativo.
3. Descomprímelo y mueve el ejecutable `axes` a un directorio en tu `PATH`.
4. Abre una **nueva terminal** y verifica con `axes --version`.

---

### `axes` en Acción: Un Vistazo al Poder

No te quedes atrás. Mientras tú buscas en el `README` de un proyecto antiguo, otros ya están orquestando.

#### 1. Scripts como Funciones de CLI

Define parámetros, valores por defecto y validación directamente en tu `.toml`.

```toml
[scripts]
deploy = "terraform apply -var 'env=<axes::params::0(default='staging')>'"
```

```sh
axes . deploy                # -> terraform apply -var 'env=staging'
axes . deploy production     # -> terraform apply -var 'env=production'
```

#### 2. Orquestación Multiplataforma sin Esfuerzo

Define un comando una vez, y funcionará en todo tu equipo.

```toml
[scripts.browse]
desc = "Abre la documentación local en el navegador."
windows = "start http://localhost:8080"
macos = "open http://localhost:8080"
linux = "xdg-open http://localhost:8080"
```

```sh
# Un comando para dominarlos a todos.
$ axes . browse
```

No más `if (os == "win32")` en tus scripts. `axes` te da una capa de abstracción para el sistema operativo.

#### 3. Composición y Reutilización

Construye flujos de trabajo complejos a partir de piezas simples.

```toml
[scripts]
build = "npm run build"
test = "npm run test"
quality = ["<axes::scripts::test>", "<axes::scripts::build>"]
```

```sh
axes . quality  # Ejecuta los tests y LUEGO el build.
```

#### 4. Sesiones de Enfoque Inmersivo

Sumérgete en un sub-proyecto. `axes` configura tu entorno por ti.

```toml
# en mi-app/api/.axes/axes.toml
[options]
at_start = "source .venv/bin/activate" # Se ejecuta al entrar
at_exit = "docker-compose down"       # Se ejecuta al salir
```

```sh
$ axes mi-app/api # Inicia la sesión. `at_start` se ejecuta automáticamente.

(axes: mi-app/api) $ axes test  # No necesitas repetir el contexto.
(axes: mi-app/api) $ exit       # `at_exit` se ejecuta automáticamente.
```

**Tu entorno de desarrollo, bajo demanda.**

#### 4. Orientado al rendimiento

`axes` está escrito en Rust y busca ofrecer todas estas potentes características con el minimo gasto de recursos posible. Con un **caché perezoso y persistente**, la primera ejecución de un script complejo puede tardar más tiempo, pero las siguientes serán infinitamente más rapidas, e imperceptibles, haciendo del CI/CD infinitamente más potente.

---

### ¿Listo para dirigir tu propia orquesta?

La fricción que sientes cada día no es un requisito del desarrollo de software. Es un problema que tiene solución. `axes` es esa solución.

- ➡️ **[Guía de Inicio Rápido (`GETTING_STARTED.md`)](./GETTING_STARTED.md):** Tu tutorial paso a paso para construir tu primer monorepo orquestado en 15 minutos.
- 📖 **[Dominando el `axes.toml` (`AXES_TOML_GUIDE.md`)](./AXES_TOML_GUIDE.md):** La referencia definitiva de cada característica.
- ⌨️ **[Referencia de Comandos (`COMMAND.md`)](./COMMAND.md):** Una guía completa de cada comando de la CLI.

### Únete a la Revolución del Flujo de Trabajo

`axes` es más que una herramienta; es un movimiento para devolver el control y la coherencia a los desarrolladores. Pero no podemos hacerlo solos.

Ya seas un programador novato buscando orden en tus proyectos personales, un desarrollador senior optimizando el `CI/CD` de tu empresa, o un equipo independiente que necesita un lenguaje común, tu voz importa.

- **Encuentra un Bug o tienes una Idea Genial:** [**Abre un Issue**](https://github.com/RetypeOS/axes/issues)
- **Quieres Contribuir con Código:** ¡Los Pull Requests son bienvenidos!

**Instala `axes` hoy. Deja de buscar comandos. Olvídate de los pequeños problemas. Céntrate en lo que realmente importa: **darle vida a tu software**, y deja que `axes` se preocupe del cómo.**
