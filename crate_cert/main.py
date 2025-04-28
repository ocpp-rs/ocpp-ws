from ca import create_ca, sign_csr
from requester import generate_private_key, generate_csr
from verify import verify_certificate
from server import run_server
from client import run_client
import threading
import time
import os


def setup_pki():
    print("\n1. Creating Certificate Authority...")
    ca_key, ca_cert = create_ca()
    
    print("\n2. Generating keys and CSRs for requesters A and B...")
    # Requester A
    a_key = generate_private_key("a_key.pem")
    a_csr = generate_csr(a_key, "a.demo.com", csr_file="a.csr")
    
    # Requester B
    b_key = generate_private_key("b_key.pem")
    b_csr = generate_csr(b_key, "b.demo.com", csr_file="b.csr")
    
    print("\n3. CA signing certificates for requesters A and B...")
    a_cert = sign_csr(ca_key, ca_cert, "a.csr", "a_cert.pem")
    b_cert = sign_csr(ca_key, ca_cert, "b.csr", "b_cert.pem")
    
    print("\n4. Verifying certificates...")
    verify_certificate("a_cert.pem", "ca_cert.pem")
    verify_certificate("b_cert.pem", "ca_cert.pem")


def run_server_client_demo():
    print("\n5. Setting up mutual TLS authentication WebSocket server and client...")
    # Start server in a thread
    server_thread = threading.Thread(target=run_server)
    server_thread.daemon = True
    server_thread.start()
    
    # Wait for server to start
    print("Starting server...")
    time.sleep(2)
    
    # Run client
    print("\n6. Client connecting to server with mutual certificate verification...")
    run_client()


def list_files():
    print("\nGenerated PKI files:")
    for file in sorted(os.listdir()):
        if file.endswith(".pem") or file.endswith(".csr"):
            print(f"- {file}")


def run_interactive_demo():
    """Run an interactive demo with the client"""
    print("\nStarting interactive WebSocket demo...")
    # Start server in a thread
    server_thread = threading.Thread(target=run_server)
    server_thread.daemon = True
    server_thread.start()
    
    # Wait for server to start
    print("Starting server...")
    time.sleep(2)
    
    # Run client in interactive mode
    print("\nStarting interactive client...")
    run_client(interactive=True)


if __name__ == "__main__":
    print("PKI Demonstration with Python")
    print("============================")
    
    # Check if PKI files already exist
    if not os.path.exists("ca_cert.pem"):
        setup_pki()
    else:
        print("\nUsing existing PKI files...")
    
    # Display demo options
    print("\nSelect a demo mode:")
    print("1. Run automated WebSocket demo")
    print("2. Run interactive WebSocket demo")
    
    # Use a default choice (1) for non-interactive environments
    choice = "1"
    try:
        choice = input("Enter choice (1/2) [default=1]: ")
        if not choice:
            choice = "1"
    except (EOFError, KeyboardInterrupt):
        print("\nUsing default choice: 1 (automated demo)")
        choice = "1"
    
    if choice == "2":
        run_interactive_demo()
    else:
        run_server_client_demo()
    
    list_files()
    
    print("\nDemonstration completed.")