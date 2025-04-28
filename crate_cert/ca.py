from cryptography import x509
from cryptography.x509.oid import NameOID
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
import datetime
import os


def create_ca():
    # Generate CA's private key
    private_key = rsa.generate_private_key(
        public_exponent=65537,
        key_size=2048,
    )
    
    # Store CA's private key
    with open("ca_key.pem", "wb") as f:
        f.write(private_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption()
        ))
    
    # Create CA's self-signed certificate
    subject = issuer = x509.Name([
        x509.NameAttribute(NameOID.COUNTRY_NAME, u"US"),
        x509.NameAttribute(NameOID.STATE_OR_PROVINCE_NAME, u"California"),
        x509.NameAttribute(NameOID.LOCALITY_NAME, u"San Francisco"),
        x509.NameAttribute(NameOID.ORGANIZATION_NAME, u"Demo CA"),
        x509.NameAttribute(NameOID.COMMON_NAME, u"ca.demo.com"),
    ])
    
    certificate = x509.CertificateBuilder().subject_name(
        subject
    ).issuer_name(
        issuer
    ).public_key(
        private_key.public_key()
    ).serial_number(
        x509.random_serial_number()
    ).not_valid_before(
        datetime.datetime.utcnow()
    ).not_valid_after(
        datetime.datetime.utcnow() + datetime.timedelta(days=365)
    ).add_extension(
        x509.BasicConstraints(ca=True, path_length=None), critical=True
    ).add_extension(
        x509.KeyUsage(digital_signature=True, content_commitment=False,
                     key_encipherment=False, data_encipherment=False,
                     key_agreement=False, key_cert_sign=True,
                     crl_sign=True, encipher_only=False, decipher_only=False), critical=True
    ).sign(private_key, hashes.SHA256())
    
    # Store CA's certificate
    with open("ca_cert.pem", "wb") as f:
        f.write(certificate.public_bytes(serialization.Encoding.PEM))
    
    return private_key, certificate


def sign_csr(ca_private_key, ca_cert, csr_file, cert_file):
    # Load the CSR
    with open(csr_file, "rb") as f:
        csr = x509.load_pem_x509_csr(f.read())
    
    # Get the SAN extension from CSR if it exists
    san_extension = None
    try:
        san_extension = csr.extensions.get_extension_for_class(x509.SubjectAlternativeName)
    except x509.ExtensionNotFound:
        pass

    # Create certificate builder
    builder = x509.CertificateBuilder().subject_name(
        csr.subject
    ).issuer_name(
        ca_cert.subject
    ).public_key(
        csr.public_key()
    ).serial_number(
        x509.random_serial_number()
    ).not_valid_before(
        datetime.datetime.utcnow()
    ).not_valid_after(
        datetime.datetime.utcnow() + datetime.timedelta(days=365)
    )

    # Add basic extensions
    builder = builder.add_extension(
        x509.BasicConstraints(ca=False, path_length=None), critical=True
    ).add_extension(
        x509.KeyUsage(digital_signature=True, content_commitment=False,
                     key_encipherment=True, data_encipherment=False,
                     key_agreement=False, key_cert_sign=False,
                     crl_sign=False, encipher_only=False, decipher_only=False), critical=True
    ).add_extension(
        x509.ExtendedKeyUsage([x509.oid.ExtendedKeyUsageOID.SERVER_AUTH,
                             x509.oid.ExtendedKeyUsageOID.CLIENT_AUTH]), critical=False
    )

    # Copy SAN extension from CSR if it exists
    if san_extension:
        builder = builder.add_extension(
            san_extension.value,
            critical=False
        )

    # Sign the certificate
    cert = builder.sign(ca_private_key, hashes.SHA256())
    
    # Store the certificate
    with open(cert_file, "wb") as f:
        f.write(cert.public_bytes(serialization.Encoding.PEM))
    
    return cert
