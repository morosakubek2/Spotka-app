import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import '../shared/theme.dart';

void main() async {
  WidgetsFlutterBinding.ensureInitialized();
  
  // Initialize crypto manager and load/generate keys
  // Initialize local database with encryption
  // Start P2P node (for Free version)
  
  runApp(const SpotkaApp());
}

class SpotkaApp extends StatelessWidget {
  const SpotkaApp({super.key});

  @override
  Widget build(BuildContext context) {
    return ProviderScope(
      child: MaterialApp.router(
        title: 'Spotka',
        debugShowCheckedModeBanner: false,
        theme: SpotkaTheme.darkTheme,
        routerConfig: AppRouter.router,
      ),
    );
  }
}
