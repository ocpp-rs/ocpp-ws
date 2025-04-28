from cryptography import x509
from cryptography.x509.oid import NameOID
from cryptography.hazmat.primitives import hashes, serialization
from cryptography.hazmat.primitives.asymmetric import rsa
import datetime
import ipaddress


def generate_private_key(key_file):
    private_key = rsa.generate_private_key(
        public_exponent=65537,
        key_size=2048,
    )
    
    with open(key_file, "wb") as f:
        f.write(private_key.private_bytes(
            encoding=serialization.Encoding.PEM,
            format=serialization.PrivateFormat.PKCS8,
            encryption_algorithm=serialization.NoEncryption()
        ))
    
    return private_key


def generate_csr(private_key, common_name, ip_addresses=["127.0.0.1", "127.0.0.2"], country="US", state="California", locality="San Francisco",
                organization="Demo Corp", csr_file=None):
    builder = x509.CertificateSigningRequestBuilder().subject_name(x509.Name([
        x509.NameAttribute(NameOID.COUNTRY_NAME, country),
        x509.NameAttribute(NameOID.STATE_OR_PROVINCE_NAME, state),
        x509.NameAttribute(NameOID.LOCALITY_NAME, locality),
        x509.NameAttribute(NameOID.ORGANIZATION_NAME, organization),
        x509.NameAttribute(NameOID.COMMON_NAME, common_name),
    ]))
    
    if ip_addresses:
        san_list = []
        for ip in ip_addresses:
            san_list.append(x509.IPAddress(ipaddress.ip_address(ip)))
        builder = builder.add_extension(
            x509.SubjectAlternativeName(san_list),
            critical=False
        )
    
    csr = builder.sign(private_key, hashes.SHA256())
    
    if csr_file:
        with open(csr_file, "wb") as f:
            f.write(csr.public_bytes(serialization.Encoding.PEM))
    
    return csr
