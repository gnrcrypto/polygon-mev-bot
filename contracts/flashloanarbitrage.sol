// SPDX-License-Identifier: MIT
pragma solidity ^0.8.19;

import "@uniswap/v3-core/contracts/interfaces/IUniswapV3Pool.sol";
import "@uniswap/v3-core/contracts/interfaces/callback/IUniswapV3FlashCallback.sol";
import "@uniswap/v3-periphery/contracts/libraries/PoolAddress.sol";
import "@uniswap/v3-periphery/contracts/interfaces/ISwapRouter.sol";
import "@openzeppelin/contracts/token/ERC20/IERC20.sol";
import "@openzeppelin/contracts/access/Ownable.sol";

interface IAtlasSolver {
    function atlasSolverCall(
        address solverOpFrom,
        address executionEnvironment,
        address bidToken,
        uint256 bidAmount,
        bytes calldata solverOpData,
        bytes calldata extraReturnData
    ) external payable;
}

interface IFastLaneSender {
    function sendTransaction(bytes calldata data, uint256 targetBlock) external payable returns (bytes32);
}

contract FlashLoanArbitrage is IUniswapV3FlashCallback, Ownable, IAtlasSolver {
    ISwapRouter public immutable swapRouter;
    address public immutable WETH;
    address public immutable factory;
    address public immutable atlas;
    address public fastLaneSender;
    uint256 public maxDelayBlocks = 5;

    event SolverCalled(
        address solverOpFrom,
        address executionEnvironment,
        address bidToken,
        uint256 bidAmount
    );

    event FlashLoanFailed(
        address pool,
        uint256 amount0,
        uint256 amount1,
        string reason
    );

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

    struct SolverCallParams {
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
        address token0;
        address token1;
        uint256 amount0;
        uint256 amount1;
        uint24 fee;
        address[] path;
        uint256[] amounts;
        address[] routers;
    }

    struct FastLaneBundle {
        bytes data;
        uint256 targetBlock;
    }

    constructor(
        address _swapRouter,
        address _weth,
        address _factory,
        address _atlas
    ) Ownable(msg.sender) {
        swapRouter = ISwapRouter(_swapRouter);
        WETH = _weth;
        factory = _factory;
        atlas = _atlas;
    }

    function setFastLaneSender(address _fastLaneSender) external onlyOwner {
        fastLaneSender = _fastLaneSender;
    }

    function setMaxDelayBlocks(uint256 _maxDelayBlocks) external onlyOwner {
        maxDelayBlocks = _maxDelayBlocks;
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
        _executeFlashLoanArbitrage(token0, token1, amount0, amount1, fee, path, amounts, routers);
    }

    function _executeFlashLoanArbitrage(
        address token0,
        address token1,
        uint256 amount0,
        uint256 amount1,
        uint24 fee,
        address[] memory path,
        uint256[] memory amounts,
        address[] memory routers
    ) internal {
        // Get the pool address
        PoolAddress.PoolKey memory poolKey = PoolAddress.getPoolKey(token0, token1, fee);
        address poolAddress = PoolAddress.computeAddress(factory, poolKey);
        IUniswapV3Pool pool = IUniswapV3Pool(poolAddress);

        // Encode callback data
        FlashCallbackData memory data = FlashCallbackData({
            token0: token0,
            token1: token1,
            amount0: amount0,
            amount1: amount1,
            fee: fee,
            path: path,
            amounts: amounts,
            routers: routers
        });

        // Initiate the flash loan
        pool.flash(address(this), amount0, amount1, abi.encode(data));
    }

    function uniswapV3FlashCallback(
        uint256 fee0,
        uint256 fee1,
        bytes calldata data
    ) external override {
        // Decode callback data
        FlashCallbackData memory decoded = abi.decode(data, (FlashCallbackData));

        // Verify callback is from the expected pool
        PoolAddress.PoolKey memory poolKey = PoolAddress.getPoolKey(
            decoded.token0,
            decoded.token1,
            decoded.fee
        );
        address poolAddress = PoolAddress.computeAddress(factory, poolKey);
        require(msg.sender == poolAddress, "Callback not from expected pool");

        // Execute the arbitrage with try-catch for error handling
        try this.executeArbitrageInternal(decoded.path, decoded.amounts, decoded.routers) {
            // Success case - nothing to do
        } catch (bytes memory reason) {
            emit FlashLoanFailed(msg.sender, decoded.amount0, decoded.amount1, string(reason));
            revert("Arbitrage execution failed");
        }

        // Repay the flash loan with fees
        uint256 amount0Owed = decoded.amount0 + fee0;
        uint256 amount1Owed = decoded.amount1 + fee1;

        if (amount0Owed > 0) {
            IERC20(decoded.token0).transfer(msg.sender, amount0Owed);
        }
        if (amount1Owed > 0) {
            IERC20(decoded.token1).transfer(msg.sender, amount1Owed);
        }
    }

    // New helper function for try-catch to work correctly
    function executeArbitrageInternal(
        address[] memory path,
        uint256[] memory amounts,
        address[] memory routers
    ) external {
        _executeArbitrage(path, amounts, routers);
    }

    function _executeArbitrage(
        address[] memory path,
        uint256[] memory amounts,
        address[] memory routers
    ) internal {
        require(path.length >= 2, "Invalid path length");
        require(amounts.length == path.length - 1, "Invalid amounts length");
        require(routers.length == path.length - 1, "Invalid routers length");

        // Approve tokens for all routers
        for (uint256 i = 0; i < path.length - 1; i++) {
            IERC20(path[i]).approve(routers[i], amounts[i]);
        }

        // Execute swaps
        for (uint256 i = 0; i < path.length - 1; i++) {
            address router = routers[i];
            address tokenIn = path[i];
            address tokenOut = path[i + 1];
            uint256 amountIn = amounts[i];

            // Execute swap based on router type
            // This is a simplified example - you'd need to implement the actual swap logic
            // based on the router's interface (Uniswap V2, V3, Sushiswap, etc.)
            
            // Example for Uniswap V3 Router:
            if (router == address(swapRouter)) {
                ISwapRouter.ExactInputSingleParams memory params = ISwapRouter.ExactInputSingleParams({
                    tokenIn: tokenIn,
                    tokenOut: tokenOut,
                    fee: 3000, // Default fee, could be parameterized
                    recipient: address(this),
                    deadline: block.timestamp,
                    amountIn: amountIn,
                    amountOutMinimum: 0, // No slippage protection in this example
                    sqrtPriceLimitX96: 0
                });
                swapRouter.exactInputSingle(params);
            } else {
                // For other routers, you'd need to implement their specific swap logic
                // This is just a placeholder
                // Example: IUniswapV2Router(router).swapExactTokensForTokens(...);
            }
        }
    }

    function atlasSolverCall(
        address solverOpFrom,
        address executionEnvironment,
        address bidToken,
        uint256 bidAmount,
        bytes calldata solverOpData,
        bytes calldata /* extraReturnData */
    ) external payable override {
        // Basic safety checks
        require(msg.sender == atlas, "Only Atlas may call");
        require(solverOpFrom == owner(), "Invalid solverOpFrom");

        emit SolverCalled(solverOpFrom, executionEnvironment, bidToken, bidAmount);

        // Decode solverOpData into a struct
        SolverCallParams memory params = abi.decode(solverOpData, (SolverCallParams));

        // Call internal function
        _executeFlashLoanArbitrage(
            params.token0,
            params.token1,
            params.amount0,
            params.amount1,
            params.fee,
            params.path,
            params.amounts,
            params.routers
        );

        // Handle bid payment
        _handleBidPayment(executionEnvironment, bidToken, bidAmount);
    }

    function _handleBidPayment(address executionEnvironment, address bidToken, uint256 bidAmount) private {
        if (bidAmount > 0) {
            if (bidToken == address(0)) {
                require(address(this).balance >= bidAmount, "Insufficient ETH for bid");
                (bool success, ) = executionEnvironment.call{value: bidAmount}("");
                require(success, "ETH bid payment failed");
            } else {
                IERC20(bidToken).transfer(executionEnvironment, bidAmount);
            }
        }
    }

    function prepareFastLaneBundle(
        ArbitrageOpportunity memory opportunity,
        uint256 targetBlock
    ) internal pure returns (FastLaneBundle memory) {
        bytes memory callData = abi.encodeWithSelector(
            this.executeFlashLoanArbitrage.selector,
            opportunity.token0,
            opportunity.token1,
            opportunity.amount0,
            opportunity.amount1,
            opportunity.fee,
            opportunity.path,
            opportunity.amounts,
            opportunity.routers
        );
        
        return FastLaneBundle({
            data: callData,
            targetBlock: targetBlock
        });
    }

    function executeArbitrageWithFastLane(
        ArbitrageOpportunity memory opportunity,
        uint256 targetBlock
    ) external onlyOwner returns (bytes32) {
        require(targetBlock > block.number, "Invalid block number");
        require(targetBlock <= block.number + maxDelayBlocks, "Block too far");
        
        FastLaneBundle memory bundle = prepareFastLaneBundle(opportunity, targetBlock);
        
        require(fastLaneSender != address(0), "FastLane sender not set");
        
        return IFastLaneSender(fastLaneSender).sendTransaction(bundle.data, bundle.targetBlock);
    }

    function withdrawToken(address token, uint256 amount) external onlyOwner {
        IERC20(token).transfer(owner(), amount);
    }

    // Allow receiving ETH
    receive() external payable {}
}
