#include <iostream>
#include <vector>
#include <memory>

namespace GraphUtils {
    template<typename T>
    class Graph {
    private:
        std::vector<std::vector<T>> adjacencyMatrix;
        
    public:
        Graph(size_t size) : adjacencyMatrix(size, std::vector<T>(size)) {}
        
        ~Graph() = default;
        
        void addEdge(size_t from, size_t to, T weight) {
            adjacencyMatrix[from][to] = weight;
        }
        
        T getWeight(size_t from, size_t to) const {
            return adjacencyMatrix[from][to];
        }
        
        bool hasPath(size_t from, size_t to) const;
        
        void printGraph() const {
            for (const auto& row : adjacencyMatrix) {
                for (const auto& element : row) {
                    std::cout << element << " ";
                }
                std::cout << std::endl;
            }
        }
    };
    
    enum class TraversalType {
        DepthFirst,
        BreadthFirst
    };
    
    template<typename T>
    bool Graph<T>::hasPath(size_t from, size_t to) const {
        if (from == to) return true;
        
        std::vector<bool> visited(adjacencyMatrix.size(), false);
        return dfsSearch(from, to, visited);
    }
    
    template<typename T>
    bool dfsSearch(size_t current, size_t target, std::vector<bool>& visited) {
        visited[current] = true;
        
        for (size_t i = 0; i < visited.size(); ++i) {
            if (!visited[i] && adjacencyMatrix[current][i] != 0) {
                if (i == target || dfsSearch(i, target, visited)) {
                    return true;
                }
            }
        }
        return false;
    }
}

int main() {
    GraphUtils::Graph<int> graph(5);
    
    graph.addEdge(0, 1, 10);
    graph.addEdge(1, 2, 20);
    graph.addEdge(2, 3, 30);
    
    std::cout << "Graph representation:" << std::endl;
    graph.printGraph();
    
    if (graph.hasPath(0, 3)) {
        std::cout << "Path exists from 0 to 3" << std::endl;
    }
    
    return 0;
}