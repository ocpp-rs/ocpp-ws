import asyncio
import ssl
import websockets
import json
import time
from datetime import datetime

async def handle_client(websocket):
    """Handle WebSocket client connection"""
    # Get client certificate info
    cert = websocket.transport.get_extra_info('peercert')
    try:
        common_name = next((x[0][1] for x in cert['subject'] if x[0][0] == 'commonName'), None)
        client_addr = websocket.remote_address
        print(f"Connection established with {client_addr}, certificate: {common_name}")
        
        # Send welcome message
        await websocket.send(json.dumps({
            "type": "server_message",
            "message": "Hello from server! Your certificate is verified.",
            "timestamp": datetime.now().isoformat()
        }))
        
        # Echo messages
        async for message in websocket:
            try:
                data = json.loads(message)
                print(f"Received from {common_name}: {data}")
                
                # Process the message based on type
                if data.get("type") == "chat":
                    # Echo the message back with timestamp
                    response = {
                        "type": "chat",
                        "sender": "server",
                        "original_sender": data.get("sender", "unknown"),
                        "message": data.get("message", ""),
                        "timestamp": datetime.now().isoformat()
                    }
                    await websocket.send(json.dumps(response))
                elif data.get("type") == "ping":
                    # Respond to ping with pong
                    response = {
                        "type": "pong",
                        "timestamp": datetime.now().isoformat()
                    }
                    await websocket.send(json.dumps(response))
            except json.JSONDecodeError:
                # Handle plain text
                print(f"Received text from {common_name}: {message}")
                await websocket.send(json.dumps({
                    "type": "echo",
                    "message": message,
                    "timestamp": datetime.now().isoformat()
                }))
    except Exception as e:
        print(f"Error handling client {websocket.remote_address}: {e}")
    finally:
        print(f"Connection closed with {websocket.remote_address}")


async def start_server(host='localhost', port=8443, certfile='a_cert.pem', keyfile='a_key.pem', ca_cert='ca_cert.pem'):
    """Start the WebSocket server with mutual TLS authentication"""
    # Create SSL context with mutual authentication
    ssl_context = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
    ssl_context.verify_mode = ssl.CERT_REQUIRED
    ssl_context.load_cert_chain(certfile=certfile, keyfile=keyfile)
    ssl_context.load_verify_locations(cafile=ca_cert)
    
    # Start WebSocket server
    async with websockets.serve(handle_client, host, port, ssl=ssl_context):
        print(f"WebSocket server started on wss://{host}:{port}")
        # Keep the server running
        await asyncio.Future()


def run_server(host='localhost', port=8443, certfile='a_cert.pem', keyfile='a_key.pem', ca_cert='ca_cert.pem'):
    """Run the WebSocket server"""
    asyncio.run(start_server(host, port, certfile, keyfile, ca_cert))


if __name__ == "__main__":
    run_server()