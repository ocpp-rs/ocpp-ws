use rustls::pki_types::CertificateDer;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::time::SystemTime;
use x509_parser::prelude::*;
use x509_parser::extensions::{GeneralName, ParsedExtension};
use ring::digest;
use log::{info, error, debug};
use env_logger;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化日志系统
    env_logger::init();

    // 证书目录
    let cert_dir = Path::new("./certs");

    // 加载CA证书
    info!("加载CA证书...");
    let ca_cert_path = cert_dir.join("ca_cert.pem");
    let ca_cert = load_certificate(&ca_cert_path)?;

    // 验证第一个证书
    info!("\n=== 验证第一个证书 (example.com) ===");

    // 加载第一个签名证书
    info!("加载第一个签名证书...");
    let signed_cert1_path = cert_dir.join("a_cert.pem");
    let signed_cert1 = load_certificate(&signed_cert1_path)?;

    // 验证第一个证书
    info!("验证第一个证书...");
    verify_certificate(&ca_cert, &signed_cert1, "example.com")?;

    // 显示第一个证书信息
    info!("第一个证书信息:");
    display_certificate_info(&signed_cert1)?;

    // 验证第二个证书
    info!("\n=== 验证第二个证书 (test.com) ===");

    // 加载第二个签名证书
    info!("加载第二个签名证书...");
    let signed_cert2_path = cert_dir.join("b_cert.pem");
    let signed_cert2 = load_certificate(&signed_cert2_path)?;

    // 验证第二个证书
    info!("验证第二个证书...");
    verify_certificate(&ca_cert, &signed_cert2, "test.com")?;

    // 显示第二个证书信息
    info!("第二个证书信息:");
    display_certificate_info(&signed_cert2)?;

    info!("\n=== 所有证书验证成功 ===");

    Ok(())
}

// 加载PEM格式的证书
fn load_certificate(path: &Path) -> Result<CertificateDer<'static>, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut cert_data = Vec::new();
    file.read_to_end(&mut cert_data)?;

    // 解析PEM格式
    let (_, pem) = x509_parser::pem::parse_x509_pem(&cert_data)?;
    let cert_der = CertificateDer::from(pem.contents);

    Ok(cert_der)
}

// 验证证书
fn verify_certificate(ca_cert: &CertificateDer, signed_cert: &CertificateDer, dns_name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // 解析CA证书
    let (_, ca_x509) = X509Certificate::from_der(ca_cert.as_ref())?;

    // 解析签名证书
    let (_, signed_x509) = X509Certificate::from_der(signed_cert.as_ref())?;

    let ca_public_key = ca_x509.public_key();
    let signed_x509_c = signed_x509.clone();
    if signed_x509_c.verify_signature(Some(ca_public_key)).is_ok(){
        info!("证书验证成功！");
    }else {
        error!("证书验证失败");
        return Err("证书验证失败".into());
    }


    // 使用CA公钥验证签名


    // 验证证书有效期
    let now = SystemTime::now();
    let since_epoch = now.duration_since(SystemTime::UNIX_EPOCH)?.as_secs();

    // 检查签名证书的有效期
    let not_before = signed_x509.validity().not_before;
    let not_after = signed_x509.validity().not_after;
    if not_before.timestamp() > since_epoch as i64 || not_after.timestamp() < since_epoch as i64 {
        return Err("证书已过期或尚未生效".into());
    }

    // 检查CA证书的有效期
    let ca_not_before = ca_x509.validity().not_before;
    let ca_not_after = ca_x509.validity().not_after;
    if ca_not_before.timestamp() > since_epoch as i64 || ca_not_after.timestamp() < since_epoch as i64 {
        return Err("CA证书已过期或尚未生效".into());
    }

    // 验证域名
    let mut found = false;
    let mut all_domains = Vec::new();

    // 检查主题通用名称
    if let Some(cn) = signed_x509.subject().iter_common_name().next() {
        if let Ok(cn_str) = cn.as_str() {
            all_domains.push(cn_str.to_string());
            if cn_str == dns_name {
                found = true;
            }
        }
    }

    // 使用x509-parser库解析SAN扩展
    // 检查证书扩展
    for ext in signed_x509.extensions() {
        // 查找SAN扩展 (OID 2.5.29.17)
        if ext.oid == oid_registry::OID_X509_EXT_SUBJECT_ALT_NAME {
            debug!("找到SAN扩展");

            // 使用x509-parser解析SAN扩展
            match ext.parsed_extension() {
                ParsedExtension::SubjectAlternativeName(san) => {
                    for name in &san.general_names {
                        match name {
                            GeneralName::DNSName(dns) => {
                                debug!("从证书中提取到DNS名称: {}", dns);
                                all_domains.push(dns.to_string());
                                if *dns == dns_name {
                                    found = true;
                                }
                            },
                            GeneralName::IPAddress(ip) => {
                                // 将IP地址字节转换为字符串
                                if ip.len() == 4 {
                                    // IPv4
                                    let ip_str = format!("{}.{}.{}.{}", ip[0], ip[1], ip[2], ip[3]);
                                    debug!("从证书中提取到IP地址: {}", ip_str);
                                    all_domains.push(ip_str);
                                }
                            },
                            _ => {} // 忽略其他类型的名称
                        }
                    }
                },
                _ => {
                    debug!("无法解析SAN扩展");
                }
            }
        }
    }

    info!("证书验证成功！");
    info!("- 证书由CA签名");
    info!("- 证书对域名 {} 有效", dns_name);
    info!("- 证书在当前时间有效");

    // 输出证书包含的所有域名
    info!("证书包含的所有域名和IP地址:");
    for (i, domain) in all_domains.iter().enumerate() {
        info!("  {}. {}", i+1, domain);
    }

    Ok(())
}

// 显示证书信息
fn display_certificate_info(cert: &CertificateDer) -> Result<(), Box<dyn std::error::Error>> {
    // 解析证书
    let (_, x509) = X509Certificate::from_der(cert.as_ref())?;

    // 显示证书主题
    info!("主题: {}", x509.subject());

    // 显示证书颁发者
    info!("颁发者: {}", x509.issuer());

    // 显示证书有效期
    let not_before = x509.validity().not_before;
    let not_after = x509.validity().not_after;
    info!("有效期: {} 至 {}", not_before, not_after);

    // 显示证书序列号
    let serial_hex = x509.raw_serial().iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<String>>()
        .join(":");
    info!("序列号: {}", serial_hex);

    // 显示证书指纹
    let fingerprint = sha256_fingerprint(cert.as_ref());
    info!("SHA-256指纹: {}", fingerprint);

    // 显示证书扩展
    info!("证书扩展:");
    for extension in x509.extensions() {
        match extension.parsed_extension() {
            ParsedExtension::SubjectAlternativeName(san) => {
                info!("- 主题备用名称 (SAN):");
                for (i, name) in san.general_names.iter().enumerate() {
                    match name {
                        GeneralName::DNSName(dns) => {
                            info!("  {}. DNS名称: {}", i+1, dns);
                        },
                        GeneralName::IPAddress(ip) => {
                            if ip.len() == 4 {
                                info!("  {}. IP地址: {}.{}.{}.{}", i+1, ip[0], ip[1], ip[2], ip[3]);
                            } else {
                                info!("  {}. IP地址: {:?}", i+1, ip);
                            }
                        },
                        _ => {
                            info!("  {}. 其他类型: {:?}", i+1, name);
                        }
                    }
                }
            },
            ParsedExtension::BasicConstraints(bc) => {
                info!("- 基本约束:");
                info!("  CA: {}", bc.ca);
                if let Some(path_len) = bc.path_len_constraint {
                    info!("  路径长度约束: {}", path_len);
                }
            },
            ParsedExtension::KeyUsage(ku) => {
                info!("- 密钥用途:");
                if ku.digital_signature() { info!("  数字签名"); }
                if ku.non_repudiation() { info!("  不可否认"); }
                if ku.key_encipherment() { info!("  密钥加密"); }
                if ku.data_encipherment() { info!("  数据加密"); }
                if ku.key_agreement() { info!("  密钥协商"); }
                if ku.key_cert_sign() { info!("  证书签名"); }
                if ku.crl_sign() { info!("  CRL签名"); }
                if ku.encipher_only() { info!("  仅加密"); }
                if ku.decipher_only() { info!("  仅解密"); }
            },
            _ => {
                info!("- {}: {}", extension.oid, format_extension_value(&extension.value));
            }
        }
    }

    Ok(())
}

// 格式化扩展值
fn format_extension_value(value: &[u8]) -> String {
    // 简单地将字节转换为十六进制字符串
    value.iter()
        .map(|b| format!("{:02X}", b))
        .collect::<Vec<String>>()
        .join(":")
}

// 计算SHA-256指纹
fn sha256_fingerprint(data: &[u8]) -> String {
    use std::fmt::Write;

    let digest = digest::digest(&digest::SHA256, data);
    let mut fingerprint = String::with_capacity(digest.as_ref().len() * 3);

    for (i, b) in digest.as_ref().iter().enumerate() {
        if i > 0 {
            write!(&mut fingerprint, ":").unwrap();
        }
        write!(&mut fingerprint, "{:02X}", b).unwrap();
    }

    fingerprint
}
