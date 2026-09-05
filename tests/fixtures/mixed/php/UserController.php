<?php
namespace App\Controller;
use App\Service\UserService;
class UserController {
    #[Route('/users')]
    public function index() { return UserService::all(); }
}
