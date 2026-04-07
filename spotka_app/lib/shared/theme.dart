import 'package:flutter/material.dart';
import 'package:go_router/go_router.dart';

/// Minimalist, energy-efficient UI theme for Spotka
/// No animations, no gradients, low-power color scheme

class SpotkaTheme {
  // Dark theme optimized for OLED screens (true black background)
  static const Color backgroundColor = Color(0xFF000000);
  static const Color surfaceColor = Color(0xFF121212);
  static const Color cardColor = Color(0xFF1E1E1E);
  
  // Accent colors (muted, non-distracting)
  static const Color primaryColor = Color(0xFF4A90A4); // Muted teal
  static const Color secondaryColor = Color(0xFF6B8E7A); // Sage green
  static const Color errorColor = Color(0xFFB85C5C); // Muted red
  
  // Text colors
  static const Color textPrimary = Color(0xFFE0E0E0);
  static const Color textSecondary = Color(0xFF9E9E9E);
  static const Color textDisabled = Color(0xFF616161);
  
  // Status colors
  static const Color statusPlanned = Color(0xFF4A90A4);
  static const Color statusActive = Color(0xFF6B8E7A);
  static const Color statusCompleted = Color(0xFF8B8B8B);
  static const Color statusCancelled = Color(0xFFB85C5C);
  
  static ThemeData get darkTheme {
    return ThemeData(
      useMaterial3: false, // Disable Material 3 animations
      brightness: Brightness.dark,
      scaffoldBackgroundColor: backgroundColor,
      primaryColor: primaryColor,
      colorScheme: const ColorScheme.dark(
        background: backgroundColor,
        surface: surfaceColor,
        primary: primaryColor,
        secondary: secondaryColor,
        error: errorColor,
        onBackground: textPrimary,
        onSurface: textPrimary,
        onPrimary: backgroundColor,
        onSecondary: backgroundColor,
        onError: backgroundColor,
      ),
      appBarTheme: const AppBarTheme(
        backgroundColor: backgroundColor,
        elevation: 0,
        centerTitle: true,
        titleTextStyle: TextStyle(
          color: textPrimary,
          fontSize: 18,
          fontWeight: FontWeight.w500,
        ),
      ),
      cardTheme: CardTheme(
        color: cardColor,
        elevation: 0,
        shape: RoundedRectangleBorder(
          borderRadius: BorderRadius.circular(4),
          side: const BorderSide(color: Color(0xFF2A2A2A)),
        ),
      ),
      elevatedButtonTheme: ElevatedButtonThemeData(
        style: ElevatedButton.styleFrom(
          backgroundColor: primaryColor,
          foregroundColor: backgroundColor,
          elevation: 0,
          padding: const EdgeInsets.symmetric(horizontal: 24, vertical: 12),
          shape: RoundedRectangleBorder(
            borderRadius: BorderRadius.circular(4),
          ),
          textStyle: const TextStyle(
            fontSize: 14,
            fontWeight: FontWeight.w500,
          ),
        ),
      ),
      textButtonTheme: TextButtonThemeData(
        style: TextButton.styleFrom(
          foregroundColor: primaryColor,
          padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
          textStyle: const TextStyle(
            fontSize: 14,
            fontWeight: FontWeight.w500,
          ),
        ),
      ),
      inputDecorationTheme: InputDecorationTheme(
        filled: true,
        fillColor: surfaceColor,
        contentPadding: const EdgeInsets.symmetric(horizontal: 16, vertical: 12),
        border: OutlineInputBorder(
          borderRadius: BorderRadius.circular(4),
          borderSide: const BorderSide(color: Color(0xFF2A2A2A)),
        ),
        enabledBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(4),
          borderSide: const BorderSide(color: Color(0xFF2A2A2A)),
        ),
        focusedBorder: OutlineInputBorder(
          borderRadius: BorderRadius.circular(4),
          borderSide: const BorderSide(color: primaryColor, width: 1.5),
        ),
        labelStyle: const TextStyle(color: textSecondary),
        hintStyle: const TextStyle(color: textDisabled),
      ),
      dividerTheme: const DividerThemeData(
        color: Color(0xFF2A2A2A),
        thickness: 1,
      ),
      iconTheme: const IconThemeData(
        color: textPrimary,
        size: 24,
      ),
      textTheme: const TextTheme(
        headlineLarge: TextStyle(
          color: textPrimary,
          fontSize: 24,
          fontWeight: FontWeight.w600,
        ),
        headlineMedium: TextStyle(
          color: textPrimary,
          fontSize: 20,
          fontWeight: FontWeight.w600,
        ),
        titleLarge: TextStyle(
          color: textPrimary,
          fontSize: 18,
          fontWeight: FontWeight.w500,
        ),
        titleMedium: TextStyle(
          color: textPrimary,
          fontSize: 16,
          fontWeight: FontWeight.w500,
        ),
        bodyLarge: TextStyle(
          color: textPrimary,
          fontSize: 16,
          fontWeight: FontWeight.normal,
        ),
        bodyMedium: TextStyle(
          color: textSecondary,
          fontSize: 14,
          fontWeight: FontWeight.normal,
        ),
        bodySmall: TextStyle(
          color: textDisabled,
          fontSize: 12,
          fontWeight: FontWeight.normal,
        ),
        labelLarge: TextStyle(
          color: textPrimary,
          fontSize: 14,
          fontWeight: FontWeight.w500,
        ),
      ),
      // Disable all animations for energy efficiency
      animationDuration: Duration.zero,
    );
  }
}

/// Router configuration for Spotka
class AppRouter {
  static final GoRouter router = GoRouter(
    initialLocation: '/home',
    routes: [
      GoRoute(
        path: '/home',
        name: 'home',
        builder: (context, state) => const Placeholder(), // HomeScreen
      ),
      GoRoute(
        path: '/meeting/new',
        name: 'new_meeting',
        builder: (context, state) => const Placeholder(), // NewMeetingScreen
      ),
      GoRoute(
        path: '/meeting/:id',
        name: 'meeting_details',
        builder: (context, state) => Placeholder(
          key: ValueKey(state.pathParameters['id']),
        ), // MeetingDetailsScreen
      ),
      GoRoute(
        path: '/profile',
        name: 'profile',
        builder: (context, state) => const Placeholder(), // ProfileScreen
      ),
      GoRoute(
        path: '/settings',
        name: 'settings',
        builder: (context, state) => const Placeholder(), // SettingsScreen
      ),
      GoRoute(
        path: '/premium',
        name: 'premium',
        builder: (context, state) => const Placeholder(), // PremiumScreen
      ),
    ],
  );
}
