import 'dart:convert';
import 'dart:typed_data';
import 'package:pointycastle/export.dart';
import 'package:encrypt/encrypt.dart' as encrypt_lib;
import 'package:crypto/crypto.dart';

/// Cryptographic core for Spotka App-Chain
/// Implements:
/// - Ed25519 for digital signatures (identity)
/// - X25519 for key exchange (ECDH)
/// - AES-GCM for data encryption at rest
/// - SHA-256 for hashing

class CryptoManager {
  late final Ed25519Signer _signer;
  late final ECPointGenerator _keyGenerator;
  late final SecureRandom _secureRandom;
  
  Uint8List? _privateKeyBytes;
  Uint8List? _publicKeyBytes;

  CryptoManager() {
    _secureRandom = FortunaRandom();
    _secureRandom.seed(KeyParameter(
      Uint8List.fromList(List.generate(32, (_) => DateTime.now().microsecondsSinceEpoch % 256)),
    ));
    
    _signer = Ed25519Signer();
  }

  /// Generate new Ed25519 keypair for user identity
  void generateKeypair() {
    final signer = Ed25519Signer();
    final keyPair = signer.generateKey(_secureRandom);
    
    _privateKeyBytes = (keyPair.privateKey as PrivateKey).encoded;
    _publicKeyBytes = (keyPair.publicKey as PublicKey).encoded;
    
    _signer.init(true, PrivateKeyParameter(keyPair.privateKey));
  }

  /// Load existing keys from secure storage
  void loadKeys(Uint8List privateKey, Uint8List publicKey) {
    _privateKeyBytes = privateKey;
    _publicKeyBytes = publicKey;
    
    final keyParams = PrivateKeyParameter(PrivateKey(_privateKeyBytes!));
    _signer.init(true, keyParams);
  }

  /// Get public key as hex string (user identity)
  String getPublicKeyHex() {
    if (_publicKeyBytes == null) {
      throw Exception('No public key available');
    }
    return _bytesToHex(_publicKeyBytes!);
  }

  /// Get user ID (hash of public key)
  String getUserId() {
    final publicKey = getPublicKeyHex();
    final hash = sha256.convert(utf8.encode(publicKey));
    return hash.toString();
  }

  /// Sign data with Ed25519
  Uint8List signData(Uint8List data) {
    if (_privateKeyBytes == null) {
      throw Exception('No private key available');
    }
    return _signer.generateSignature(data) as Uint8List;
  }

  /// Verify signature from another user
  bool verifySignature(Uint8List data, Uint8List signature, Uint8List publicKey) {
    final verifier = Ed25519Signer();
    final pubKey = PublicKey(publicKey);
    verifier.init(false, PublicKeyParameter(pubKey));
    
    try {
      return verifier.verifySignature(data, signature);
    } catch (e) {
      return false;
    }
  }

  /// Derive shared secret using X25519 (ECDH)
  Uint8List deriveSharedSecret(Uint8List theirPublicKey) {
    if (_privateKeyBytes == null) {
      throw Exception('No private key available');
    }

    // Convert Ed25519 to X25519 for key exchange
    // In production, use proper curve conversion or generate separate X25519 keys
    final x25519 = ECDomainParameters('curve25519');
    final privateKey = ECPrivateKey(_privateKeyBytes!, x25519);
    final theirPubKey = ECPublicKey(theirPublicKey, x25519);
    
    final generator = ECDHBasicAgreement();
    generator.init(privateKey);
    
    final sharedSecret = generator.calculateAgreement(theirPubKey);
    return sharedSecret.encoded!;
  }

  /// Encrypt data using AES-GCM with derived key
  Map<String, dynamic> encryptData(Uint8List data, Uint8List sharedSecret) {
    final key = encrypt_lib.Key(sharedSecret.sublist(0, 32));
    final iv = encrypt_lib.IV.fromLength(12); // 96-bit IV for GCM
    
    final encrypter = encrypt_lib.Encrypter(
      encrypt_lib.GCM(encrypt_lib.AES(key)),
    );
    
    final encrypted = encrypter.encryptBytes(data, iv: iv);
    
    return {
      'ciphertext': _bytesToBase64(encrypted.bytes),
      'iv': _bytesToBase64(iv.bytes),
      'authTag': _bytesToBase64(encrypted.authenticationTag!),
    };
  }

  /// Decrypt data using AES-GCM
  Uint8List decryptData(Map<String, dynamic> encryptedData, Uint8List sharedSecret) {
    final key = encrypt_lib.Key(sharedSecret.sublist(0, 32));
    final iv = encrypt_lib.IV.fromBase64(encryptedData['iv']);
    final authTag = _base64ToBytes(encryptedData['authTag']);
    final ciphertext = _base64ToBytes(encryptedData['ciphertext']);
    
    final encrypter = encrypt_lib.Encrypter(
      encrypt_lib.GCM(encrypt_lib.AES(key)),
    );
    
    final decrypted = encrypter.decryptBytes(
      encrypt_lib.Encrypted(ciphertext),
      iv: iv,
      authenticationTag: authTag,
    );
    
    return Uint8List.fromList(decrypted);
  }

  /// Hash transaction payload for App-Chain
  String hashPayload(Uint8List payload) {
    final hash = sha256.convert(payload);
    return hash.toString();
  }

  /// Create App-Chain transaction
  Map<String, dynamic> createTransaction({
    required String type,
    required Uint8List payload,
    required int timestamp,
  }) {
    final payloadHash = hashPayload(payload);
    final dataToSign = utf8.encode('$type:$payloadHash:$timestamp');
    final signature = signData(Uint8List.fromList(dataToSign));
    
    final txHash = hashPayload(Uint8List.fromList(dataToSign));
    
    return {
      'hash': txHash,
      'type': type,
      'payloadHash': payloadHash,
      'timestamp': timestamp,
      'signature': _bytesToBase64(signature),
      'signerId': getUserId(),
    };
  }

  /// Utility: Convert bytes to hex string
  String _bytesToHex(Uint8List bytes) {
    return bytes.map((b) => b.toRadixString(16).padLeft(2, '0')).join();
  }

  /// Utility: Convert bytes to base64
  String _bytesToBase64(Uint8List bytes) {
    return base64Encode(bytes);
  }

  /// Utility: Convert base64 to bytes
  Uint8List _base64ToBytes(String base64) {
    return Uint8List.fromList(base64Decode(base64));
  }
}
