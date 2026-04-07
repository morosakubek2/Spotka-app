import 'dart:async';
import 'dart:convert';
import 'dart:typed_data';
import 'package:web_socket_channel/web_socket_channel.dart';
import 'package:mdns/mdns.dart';
import '../crypto/crypto_manager.dart';

/// P2P Network Manager for Spotka (Free Version)
/// Implements decentralized communication via:
/// - mDNS for local network discovery
/// - WebSocket for direct peer-to-peer communication
/// - Gossip protocol for transaction propagation

enum PeerStatus { discovering, connected, disconnected, syncing }

class P2PManager {
  final CryptoManager cryptoManager;
  final String _serviceType = '_spotka._tcp';
  final int _port = 8080;
  
  MDNSClient? _mdnsClient;
  WebSocketChannel? _serverChannel;
  final Map<String, WebSocketChannel> _peers = {};
  final StreamController<Map<String, dynamic>> _transactionStream = 
      StreamController<Map<String, dynamic>>.broadcast();
  
  String? _localPeerId;
  bool _isRunning = false;

  P2PManager({required this.cryptoManager});

  /// Start P2P node - discover peers and accept connections
  Future<void> start() async {
    if (_isRunning) return;
    
    _localPeerId = cryptoManager.getUserId();
    _isRunning = true;
    
    // Start mDNS discovery
    await _startDiscovery();
    
    // Start local WebSocket server (for receiving connections)
    await _startServer();
    
    print('P2P node started for peer: $_localPeerId');
  }

  /// Stop P2P node
  Future<void> stop() async {
    _isRunning = false;
    
    // Close all peer connections
    for (final peer in _peers.values) {
      await peer.sink.close();
    }
    _peers.clear();
    
    // Stop mDNS
    await _mdnsClient?.close();
    
    // Stop server
    await _serverChannel?.sink.close();
    
    print('P2P node stopped');
  }

  /// Discover peers on local network via mDNS
  Future<void> _startDiscovery() async {
    _mdnsClient = MDNSClient();
    await _mdnsClient!.init();
    
    // Query for Spotka services
    final query = MDNSQuery(_serviceType);
    await _mdnsClient!.query(query);
    
    // Listen for responses
    _mdnsClient!.responses.listen((response) {
      for (final resource in response.allResources) {
        if (resource is ResourceRecordSRV) {
          final target = resource.target;
          final port = resource.port;
          _connectToPeer(target, port);
        }
      }
    });
    
    // Advertise our own service
    await _advertiseService();
  }

  /// Advertise our P2P service on the network
  Future<void> _advertiseService() async {
    // In a real implementation, use flutter_mdns or platform-specific code
    // This is a placeholder for advertising the service
    print('Advertising Spotka service on port $_port');
  }

  /// Connect to a discovered peer
  Future<void> _connectToPeer(String host, int port) async {
    try {
      final uri = Uri.parse('ws://$host:$port/ws');
      final channel = WebSocketChannel.connect(uri);
      
      // Wait for connection
      await channel.ready;
      
      // Perform handshake
      await _performHandshake(channel);
      
      // Add to peers
      final peerId = await _getPeerId(channel);
      if (peerId != null && peerId != _localPeerId) {
        _peers[peerId] = channel;
        print('Connected to peer: $peerId');
        
        // Start listening for messages
        _listenToPeer(channel, peerId);
        
        // Sync transactions
        await _syncWithPeer(channel);
      } else {
        await channel.sink.close();
      }
    } catch (e) {
      print('Failed to connect to peer $host:$port - $e');
    }
  }

  /// Start WebSocket server to accept incoming connections
  Future<void> _startServer() async {
    // Note: Dart doesn't have built-in WebSocket server
    // In production, use shelf_web_socket or platform-specific implementation
    // This is a placeholder
    print('WebSocket server would start on port $_port');
  }

  /// Perform cryptographic handshake with peer
  Future<void> _performHandshake(WebSocketChannel channel) async {
    // Send our public key
    final publicKey = cryptoManager.getPublicKeyHex();
    channel.sink.add(jsonEncode({
      'type': 'handshake',
      'publicKey': publicKey,
      'timestamp': DateTime.now().millisecondsSinceEpoch,
    }));
    
    // Wait for peer's response
    // In production, verify signature and establish shared secret
  }

  /// Get peer ID from handshake
  Future<String?> _getPeerId(WebSocketChannel channel) async {
    try {
      final response = await channel.stream.first.timeout(
        const Duration(seconds: 5),
      );
      final data = jsonDecode(response as String);
      if (data['type'] == 'handshake') {
        // Calculate peer ID from public key
        final publicKey = data['publicKey'] as String;
        // Hash the public key to get ID (simplified)
        return publicKey.substring(0, 16); // Placeholder
      }
    } catch (e) {
      print('Handshake timeout or error: $e');
    }
    return null;
  }

  /// Listen for messages from a peer
  void _listenToPeer(WebSocketChannel channel, String peerId) {
    channel.stream.listen((message) {
      try {
        final data = jsonDecode(message as String);
        _handleMessage(data, peerId);
      } catch (e) {
        print('Error parsing message from $peerId: $e');
      }
    }, onDone: () {
      print('Peer disconnected: $peerId');
      _peers.remove(peerId);
    });
  }

  /// Handle incoming message
  void _handleMessage(Map<String, dynamic> data, String fromPeerId) {
    switch (data['type']) {
      case 'transaction':
        // Received a new transaction from App-Chain
        _transactionStream.add(data);
        // Propagate to other peers (gossip)
        _propagateTransaction(data, excludePeer: fromPeerId);
        break;
        
      case 'sync_request':
        // Peer wants to sync transactions
        _sendTransactions(data['since'], fromPeerId);
        break;
        
      case 'meeting_invite':
      case 'meeting_update':
      case 'trust_cert':
        // Forward to appropriate handler
        _transactionStream.add(data);
        break;
    }
  }

  /// Broadcast transaction to all peers
  void broadcastTransaction(Map<String, dynamic> transaction) {
    for (final peer in _peers.entries) {
      try {
        peer.value.sink.add(jsonEncode(transaction));
      } catch (e) {
        print('Failed to send to ${peer.key}: $e');
      }
    }
  }

  /// Propagate transaction using gossip protocol
  void _propagateTransaction(Map<String, dynamic> transaction, {String? excludePeer}) {
    for (final peer in _peers.entries) {
      if (peer.key != excludePeer) {
        try {
          peer.value.sink.add(jsonEncode(transaction));
        } catch (e) {
          print('Failed to propagate to ${peer.key}: $e');
        }
      }
    }
  }

  /// Sync transactions with a peer
  Future<void> _syncWithPeer(WebSocketChannel channel) async {
    channel.sink.add(jsonEncode({
      'type': 'sync_request',
      'since': 0, // Request all transactions
    }));
  }

  /// Send transactions to requesting peer
  void _sendTransactions(int sinceTimestamp, String peerId) {
    // In production, query local database for transactions since timestamp
    // and send them to the peer
    print('Sending transactions since $sinceTimestamp to $peerId');
  }

  /// Stream of incoming transactions
  Stream<Map<String, dynamic>> get transactionStream => _transactionStream.stream;

  /// Get connected peers count
  int get connectedPeersCount => _peers.length;

  /// Get peer status
  PeerStatus get status {
    if (!_isRunning) return PeerStatus.disconnected;
    if (_peers.isEmpty) return PeerStatus.discovering;
    return PeerStatus.connected;
  }
}
