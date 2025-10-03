# tests/test_app.py

import pytest
from app import app as flask_app

@pytest.fixture
def client():
    """Configura un cliente de prueba para la aplicación Flask."""
    flask_app.config['TESTING'] = True
    with flask_app.test_client() as client:
        yield client

def test_main_endpoint(client):
    """
    Prueba que el endpoint '/' devuelva un código 200 (OK) y que
    contenga el saludo personalizado inyectado por axes.
    """
    response = client.get('/')
    assert response.status_code == 200
    # Comprobamos que el saludo de nuestra variable de entorno está en la respuesta
    assert b"Hello from the axes-powered API!" in response.data