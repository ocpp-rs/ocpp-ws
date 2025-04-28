#!/usr/bin/env python3
"""
Standalone script to run the WebSocket client with certificate B
"""
from client import run_client
import os
import argparse

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Run WebSocket client with TLS certificate')
    parser.add_argument('--host', default='localhost', help='Server host (default: localhost)')
    parser.add_argument('--port', type=int, default=8443, help='Server port (default: 8443)')
    parser.add_argument('--cert', default='b_cert.pem', help='Client certificate file')
    parser.add_argument('--key', default='b_key.pem', help='Client private key file')
    parser.add_argument('--ca', default='ca_cert.pem', help='CA certificate file for verifying server')
    parser.add_argument('--interactive', action='store_true', help='Run in interactive mode')
    
    args = parser.parse_args()
    
    # Check if all required files exist
    for file_path in [args.cert, args.key, args.ca]:
        if not os.path.exists(file_path):
            print(f"Error: {file_path} not found!")
            print("Please run main.py first to generate the PKI files.")
            exit(1)
    
    print(f"Connecting to WebSocket server at wss://{args.host}:{args.port}")
    print(f"Using certificate: {args.cert}")
    print(f"Using CA certificate: {args.ca} for server verification")
    
    if args.interactive:
        print("Running in interactive mode. Type messages to send to the server.")
    
    try:
        run_client(host=args.host, port=args.port, certfile=args.cert, keyfile=args.key, 
                   ca_cert=args.ca, interactive=args.interactive)
    except KeyboardInterrupt:
        print("\nClient stopped by user")