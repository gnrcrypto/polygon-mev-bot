// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

// Replace these GitHub imports
import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Pool.sol";
import "@uniswap/v3-core/contracts/interfaces/callback/IUniswapV3FlashCallback.sol";
import "@uniswap/v3-periphery/contracts/libraries/PoolAddress.sol";
import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

contract FlashLoanArbitrage is IUniswapV3FlashCallback, Ownable {
    ISwapRouter public immutable swapRouter;
    address public immutable WETH;
    address public immutable factory;
    
    struct FlashCallbackData {
        address token0;
        address token1;
        uint256 amount0;
        uint256 amount1;
        uint24 fee;
        address[] path;
        uint256[] amounts;
        address[] routers;
    }
    
    struct ArbitrageOpportunity {
        address tokenIn;
        address tokenOut;
        uint256 amountIn;
        uint256 minProfit;
        address[] path;
        address[] routers;
    }
    
    event ArbitrageExecuted(
        address indexed tokenIn,
        address indexed tokenOut,
        uint256 amountIn,
        uint256 profit,
        uint256 gasCost
    );
    
    event FlashLoanFailed(address indexed pool, uint256 amount0, uint256 amount1);
    
    constructor(
        address _swapRouter, 
        address _weth, 
        address _factory
    ) Ownable(msg.sender) { // Pass msg.sender as the initial owner
        swapRouter = ISwapRouter(_swapRouter);
        WETH = _weth;
        factory = _factory;
    }
    
    function uniswapV3FlashCallback(
        uint256 fee0,
        uint256 fee1,
        bytes calldata data
    ) external override {
        FlashCallbackData memory decoded = abi.decode(data, (FlashCallbackData));
        
        require(
            msg.sender == address(
                IUniswapV3Pool(
                    PoolAddress.computeAddress(
                        factory,
                        PoolAddress.PoolKey({
                            token0: decoded.token0,
                            token1: decoded.token1,
                            fee: decoded.fee
                        })
                    )
                )
            ),
            "Unauthorized"
        );
        
        // Execute arbitrage
        uint256 initialBalance = IERC20(decoded.token0).balanceOf(address(this));
        _executeArbitrage(decoded.path, decoded.amounts, decoded.routers);
        
        // Repay flash loan
        uint256 balance0 = IERC20(decoded.token0).balanceOf(address(this));
        uint256 balance1 = IERC20(decoded.token1).balanceOf(address(this));
        
        require(
            balance0 >= decoded.amount0 + fee0,
            "Flash loan not repaid"
        );
        require(
            balance1 >= decoded.amount1 + fee1,
            "Flash loan not repaid"
        );
        
        IERC20(decoded.token0).transfer(msg.sender, decoded.amount0 + fee0);
        IERC20(decoded.token1).transfer(msg.sender, decoded.amount1 + fee1);
        
        // Transfer profit to owner
        uint256 profit = balance0 - (decoded.amount0 + fee0);
        if (profit > 0) {
            IERC20(decoded.token0).transfer(owner(), profit);
        }
    }
    
    function executeFlashLoanArbitrage(
        address token0,
        address token1,
        uint256 amount0,
        uint256 amount1,
        uint24 fee,
        address[] calldata path,
        uint256[] calldata amounts,
        address[] calldata routers
    ) external onlyOwner {
        bytes memory data = abi.encode(
            FlashCallbackData({
                token0: token0,
                token1: token1,
                amount0: amount0,
                amount1: amount1,
                fee: fee,
                path: path,
                amounts: amounts,
                routers: routers
            })
        );
        
        IUniswapV3Pool pool = IUniswapV3Pool(
            PoolAddress.computeAddress(
                factory,
                PoolAddress.PoolKey({
                    token0: token0,
                    token1: token1,
                    fee: fee
                })
            )
        );
        
        pool.flash(address(this), amount0, amount1, data);
    }
    
    function _executeArbitrage(
        address[] memory path,
        uint256[] memory amounts,
        address[] memory routers
    ) internal {
        require(path.length >= 2, "Invalid path");
        require(path.length == amounts.length + 1, "Invalid amounts");
        require(path.length == routers.length + 1, "Invalid routers");
        
        for (uint256 i = 0; i < path.length - 1; i++) {
            address tokenIn = path[i];
            address tokenOut = path[i + 1];
            uint256 amountIn = amounts[i];
            address router = routers[i];
            
            IERC20(tokenIn).approve(router, amountIn);
            
            ISwapRouter(router).exactInputSingle(
                ISwapRouter.ExactInputSingleParams({
                    tokenIn: tokenIn,
                    tokenOut: tokenOut,
                    fee: 3000, // 0.3% fee
                    recipient: address(this),
                    deadline: block.timestamp + 300,
                    amountIn: amountIn,
                    amountOutMinimum: 0,
                    sqrtPriceLimitX96: 0
                })
            );
        }
    }
    
    function withdrawToken(address token, uint256 amount) external onlyOwner {
        IERC20(token).transfer(owner(), amount);
    }
    
    receive() external payable {}
}
