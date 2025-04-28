import asyncio
import ssl
import websockets
import json
import time
from datetime import datetime

async def connect_to_server(host='localhost', port=8443, certfile='b_cert.pem', keyfile='b_key.pem', 
                            ca_cert='ca_cert.pem', messages=None):
    """Connect to WebSocket server with mutual TLS authentication"""
    # Create SSL context
    ssl_context = ssl.create_default_context(ssl.Purpose.SERVER_AUTH)
    ssl_context.load_cert_chain(certfile=certfile, keyfile=keyfile)
    ssl_context.load_verify_locations(cafile=ca_cert)
    ssl_context.check_hostname = False  # Disable hostname checking for demo
    
    # Default messages to send if none provided
    if messages is None:
        messages = [
            {"type": "chat", "sender": "client", "message": "Hello server!"},
            {"type": "chat", "sender": "client", "message": "This is a test message from client B"},
            {"type": "ping"}
        ]
    
    uri = f"wss://{host}:{port}"
    try:
        async with websockets.connect(uri, ssl=ssl_context) as websocket:
            print(f"Connected to {uri}")
            
            # Get server certificate info
            server_cert = websocket.transport.get_extra_info('peercert')
            common_name = next((x[0][1] for x in server_cert['subject'] if x[0][0] == 'commonName'), None)
            print(f"Server certificate: {common_name}")
            
            # Receive welcome message
            response = await websocket.recv()
            print(f"Server response: {response}")
            
            # Send messages
            for msg in messages:
                message_json = json.dumps(msg)
                print(f"Sending: {message_json}")
                await websocket.send(message_json)
                
                # Wait for response
                response = await websocket.recv()
                print(f"Received: {response}")
                
                # Wait a bit between messages
                await asyncio.sleep(1)
            
            # Custom interactive mode
            if messages == []:
                print("\nInteractive mode. Type 'exit' to quit.\n")
                while True:
                    user_input = input("> ")
                    if user_input.lower() == 'exit':
                        break
                    
                    # Send as chat message
                    message = {
                        "type": "chat",
                        "sender": "client",
                        "message": user_input,
                        "timestamp": datetime.now().isoformat()
                    }
                    await websocket.send(json.dumps(message))
                    
                    # Wait for response
                    response = await websocket.recv()
                    print(f"Server: {response}")
            
            return True
    except Exception as e:
        print(f"Error connecting to server: {e}")
        return False


def run_client(host='localhost', port=8443, certfile='b_cert.pem', keyfile='b_key.pem', 
               ca_cert='ca_cert.pem', interactive=False):
    """Run the WebSocket client"""
    messages = [] if interactive else None
    asyncio.run(connect_to_server(host, port, certfile, keyfile, ca_cert, messages))


if __name__ == "__main__":
    # Run in interactive mode if called directly
    run_client(interactive=True)