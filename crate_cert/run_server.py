#!/usr/bin/env python3
"""
Standalone script to run the WebSocket server with certificate A
"""
from server import run_server
import os
import argparse

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Run WebSocket server with TLS certificate')
    parser.add_argument('--host', default='localhost', help='Server host (default: localhost)')
    parser.add_argument('--port', type=int, default=8443, help='Server port (default: 8443)')
    parser.add_argument('--cert', default='a_cert.pem', help='Server certificate file')
    parser.add_argument('--key', default='a_key.pem', help='Server private key file')
    parser.add_argument('--ca', default='ca_cert.pem', help='CA certificate file for verifying clients')
    
    args = parser.parse_args()
    
    # Check if all required files exist
    for file_path in [args.cert, args.key, args.ca]:
        if not os.path.exists(file_path):
            print(f"Error: {file_path} not found!")
            print("Please run main.py first to generate the PKI files.")
            exit(1)
    
    print(f"Starting WebSocket server on wss://{args.host}:{args.port}")
    print(f"Using certificate: {args.cert}")
    print(f"Using CA certificate: {args.ca} for client verification")
    
    try:
        run_server(host=args.host, port=args.port, certfile=args.cert, keyfile=args.key, ca_cert=args.ca)
    except KeyboardInterrupt:
        print("\nServer stopped by user")