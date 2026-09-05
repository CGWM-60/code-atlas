import 'package:flutter/widgets.dart';
final usersProvider = FutureProvider((ref) => fetchUsers());
class UsersPage extends StatelessWidget { Widget build(context) => Text('users'); }
final router = GoRouter(routes: [GoRoute(path: '/users', builder: (_, __) => UsersPage())]);
