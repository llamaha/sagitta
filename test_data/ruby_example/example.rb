# test_data/ruby_example/example.rb

class Greeter
  def initialize(name)
    @name = name
  end

  def greet
    puts "Hello, #{@name}! This is a class definition."
  end
end

# Create an instance and greet
g = Greeter.new("World")
g.greet 