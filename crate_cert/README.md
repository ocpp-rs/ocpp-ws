# Python PKI Demonstration

A demonstration of Public Key Infrastructure (PKI) using Python with WebSockets for secure communication.

## Features

1. Certificate Authority (CA) creation
2. Certificate Signing Request (CSR) generation for two entities (A and B)
3. Certificate signing by the CA
4. Certificate verification
5. Mutual TLS authentication with WebSocket communication between:
   - Server (using A's certificate) 
   - Client (using B's certificate)

## Requirements

- Python 3.6+
- Required packages:
  ```
  pip install -r requirements.txt
  ```

## Project Structure

- `ca.py` - Certificate Authority implementation
- `requester.py` - Certificate requester implementation
- `verify.py` - Certificate verification
- `server.py` - WebSocket server using certificate A
- `client.py` - WebSocket client using certificate B
- `main.py` - Main demonstration script
- `run_server.py` - Standalone script for running the server
- `run_client.py` - Standalone script for running the client

## WebSocket Communication

The server and client communicate using WebSockets with JSON messages. Message types include:

- `chat`: Text messages that are echoed back
- `ping`/`pong`: Simple heartbeat messages
- `server_message`: Server notifications

All messages include timestamps and the server keeps track of the sender's identity using their TLS certificate.

## Running the Demo

### Full Demonstration

```bash
python main.py
```

The demonstration will ask you to select a mode:
1. **Automated Demo**: Demonstrates the PKI setup and sends a few predefined messages
2. **Interactive Demo**: Allows you to type messages to send to the server

The demonstration will:
1. Create a Certificate Authority (if certificates don't exist)
2. Generate keys and CSRs for requesters A and B
3. Sign the CSRs with the CA
4. Verify the certificates
5. Start a WebSocket server using A's certificate
6. Connect to the server using a client with B's certificate (mutual TLS authentication)
7. Exchange messages securely

### Running Server and Client Separately

After generating the certificates with `main.py`, you can run the server and client separately:

#### Server

```bash
./run_server.py [--host HOST] [--port PORT] [--cert CERT_FILE] [--key KEY_FILE] [--ca CA_FILE]
```

#### Client

```bash
./run_client.py [--host HOST] [--port PORT] [--cert CERT_FILE] [--key KEY_FILE] [--ca CA_FILE] [--interactive]
```

Use the `--interactive` flag with the client to enter an interactive message mode.

## Generated Files

After running the demo, you'll see the following files:
- `ca_key.pem` - CA private key
- `ca_cert.pem` - CA certificate
- `a_key.pem` - Requester A's private key
- `a.csr` - Requester A's certificate signing request
- `a_cert.pem` - Requester A's signed certificate
- `b_key.pem` - Requester B's private key 
- `b.csr` - Requester B's certificate signing request
- `b_cert.pem` - Requester B's signed certificate