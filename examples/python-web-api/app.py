# app.py

from flask import Flask
import os

# Crear la instancia de la aplicación Flask
app = Flask(__name__)

@app.route('/')
def hello_world():
    """
    Endpoint principal que devuelve un saludo.
    El saludo se obtiene de una variable de entorno para demostrar la
    integración con la sección `[env]` de axes.
    """
    greeting = os.getenv('API_GREETING', 'Hello, World from Flask!')
    return f"<h1>{greeting}</h1>"

# Esto permite ejecutarlo con `python app.py` si se desea,
# aunque usaremos `flask run` a través de axes.
if __name__ == '__main__':
    app.run(debug=True)