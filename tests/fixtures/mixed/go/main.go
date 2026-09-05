package main
type User struct { Name string }
type Repository interface { Save(User) }
func (u User) Display() string { return u.Name }
func main() { println(User{Name:"Ada"}.Display()) }
