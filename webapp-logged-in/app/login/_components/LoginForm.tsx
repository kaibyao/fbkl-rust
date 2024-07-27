export const LoginForm: React.FC = () => {
  return (
    <form method="POST" action="/login">
      <input type="email" name="email" placeholder="Email" />
      <input type="password" name="password" />
      <button type="submit">Submit</button>
    </form>
  );
};
