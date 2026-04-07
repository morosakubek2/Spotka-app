import 'package:drift/drift.dart';

// Local database schema for Spotka (Free version - decentralized)
// All data is encrypted at rest using AES-GCM via SQLCipher

class Users extends Table {
  TextColumn get id => text()(); // Public key hash (identity)
  TextColumn get publicKey => text()(); // Ed25519/X25519 public key
  TextColumn get displayName => text().withDefault(const Constant('Anonymous'))();
  IntColumn get reliabilityScore => integer().withDefault(const Constant(50))(); // 0-100
  DateTimeColumn get lastSeen => dateTime().nullable()();
  BoolColumn get isPremium => boolean().withDefault(const Constant(false))();
  
  @override
  List<Set<Column>> get uniqueKeys => [
    {id}
  ];
}

class Meetings extends Table {
  TextColumn get id => text()(); // UUID
  TextColumn get organizerId => text().references(Users, #id)();
  TextColumn get title => text()();
  TextColumn get description => text().nullable()();
  RealColumn get latitude => real()();
  RealColumn get longitude => real()();
  DateTimeColumn get startTime => dateTime()();
  DateTimeColumn get endTime => dateTime()();
  IntColumn get maxParticipants => integer().withDefault(const Constant(2))();
  TextColumn get status => text()(); // 'planned', 'active', 'completed', 'cancelled'
  DateTimeColumn get createdAt => dateTime()();
  DateTimeColumn get expiresAt => dateTime()(); // Auto-delete after this time
  
  @override
  List<Set<Column>> get uniqueKeys => [
    {id}
  ];
}

class MeetingParticipants extends Table {
  TextColumn get meetingId => text().references(Meetings, #id)();
  TextColumn get userId => text().references(Users, #id)();
  DateTimeColumn get joinedAt => dateTime()();
  TextColumn get status => text()(); // 'invited', 'confirmed', 'attended', 'no-show'
  IntColumn get verificationMethod => integer().nullable()(); // 0=none, 1=QR, 2=geofence, 3=biometric
  
  @override
  List<Set<Column>> get uniqueKeys => [
    {meetingId, userId}
  ];
}

class TrustCertificates extends Table {
  TextColumn get id => text()(); // Hash of certificate
  TextColumn get issuerId => text().references(Users, #id)();
  TextColumn get subjectId => text().references(Users, #id)();
  IntColumn get trustLevel => integer()(); // 0-100
  TextColumn get reason => text().nullable()();
  DateTimeColumn get issuedAt => dateTime()();
  DateTimeColumn get expiresAt => dateTime().nullable()();
  BoolColumn get isRevoked => boolean().withDefault(const Constant(false))();
  
  @override
  List<Set<Column>> get uniqueKeys => [
    {id}
  ];
}

class AppChainTransactions extends Table {
  TextColumn get hash => text()(); // Transaction hash
  TextColumn get type => text()(); // 'user_update', 'trust_cert', 'meeting_event'
  TextColumn get payloadHash => text()(); // Hash of encrypted payload
  IntColumn get timestamp => integer()(); // Unix timestamp
  TextColumn get signature => text()(); // Digital signature
  IntColumn get blockHeight => integer().nullable()(); // Optional for batching
  
  @override
  List<Set<Column>> get uniqueKeys => [
    {hash}
  ];
}

class UserSettings extends Table {
  TextColumn get userId => text().references(Users, #id)();
  TextColumn get encryptionKeySalt => text()(); // For deriving AES key
  IntColumn get visibilityWindow => integer().withDefault(const Constant(30))(); // Minutes
  BoolColumn get enableBiometric => boolean().withDefault(const Constant(false))();
  BoolColumn get autoDeleteHistory => boolean().withDefault(const Constant(true))();
  IntColumn get autoDeleteDays => integer().withDefault(const Constant(7))();
  
  @override
  List<Set<Column>> get uniqueKeys => [
    {userId}
  ];
}

@DriftDatabase(tables: [
  Users,
  Meetings,
  MeetingParticipants,
  TrustCertificates,
  AppChainTransactions,
  UserSettings,
])
class SpotkaDatabase extends _$SpotkaDatabase {
  SpotkaDatabase(super.e);
  
  @override
  int get schemaVersion => 1;
  
  @override
  MigrationStrategy get migration {
    return MigrationStrategy(
      onCreate: (Migrator m) async {
        await m.createAll();
      },
      beforeOpen: (details) async {
        // Enable foreign keys
        await customStatement('PRAGMA foreign_keys = ON');
        
        // Auto-delete old meetings
        await delete(meetings)
            .where((t) => t.expiresAt.isSmallerThanValue(DateTime.now()))
            .go();
            
        // Auto-delete old participants records
        final cutoff = DateTime.now().subtract(const Duration(days: 30));
        await delete(meetingParticipants)
            .where((t) => t.joinedAt.isSmallerThanValue(cutoff))
            .go();
      },
    );
  }
}
