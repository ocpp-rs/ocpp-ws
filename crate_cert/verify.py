from cryptography import x509
from cryptography.hazmat.primitives import serialization
from cryptography.x509 import load_pem_x509_certificate
import os


def verify_certificate(cert_file, ca_cert_file):
    # Load the certificate to verify
    with open(cert_file, "rb") as f:
        cert_data = f.read()
        cert = load_pem_x509_certificate(cert_data)
    
    # Load the CA certificate
    with open(ca_cert_file, "rb") as f:
        ca_cert_data = f.read()
        ca_cert = load_pem_x509_certificate(ca_cert_data)
    
    # Check issuer matches CA's subject
    if cert.issuer != ca_cert.subject:
        print(f"FAILED: Issuer of {os.path.basename(cert_file)} doesn't match CA's subject")
        return False
    
    # Verify certificate was signed by the CA's private key
    ca_public_key = ca_cert.public_key()
    try:
        # This will raise an exception if the verification fails
        ca_public_key.verify(
            cert.signature,
            cert.tbs_certificate_bytes,
            cert.signature_algorithm_parameters,
            cert.signature_hash_algorithm
        )
        print(f"SUCCESS: {os.path.basename(cert_file)} is verified against {os.path.basename(ca_cert_file)}")
        return True
    except Exception as e:
        print(f"FAILED: {os.path.basename(cert_file)} verification failed: {e}")
        return False